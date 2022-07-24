use crate::{Context, rust_type};
use p4::hlir::Hlir;
use p4::ast::{AST, Control, PackageInstance, Parser, Type};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub(crate) struct PipelineGenerator<'a> {
    ast: &'a AST,
    ctx: &'a mut Context,
    hlir: &'a Hlir,
}

impl <'a> PipelineGenerator<'a> {
    pub(crate) fn new(
        ast: &'a AST,
        hlir: &'a Hlir, 
        ctx: &'a mut Context
    ) -> Self {
        Self{ ast, hlir, ctx }
    }

    pub(crate) fn generate(&mut self) {

        if let Some(ref inst) = self.ast.package_instance {
            self.generate_pipeline(inst);
        }

    }

    pub(crate) fn generate_pipeline(&mut self, inst: &PackageInstance) {

        //TODO check instance against package definition instead of hardcoding
        //SoftNPU in below. This begs the question, will the Rust back end
        //support something besides SoftNPU. Probably not for the forseeable
        //future. However it could be interesting to support different package
        //structures to emulate different types of hardware. For example the
        //Tofino package instance has a relatively complex pipeline structure.
        //And it could be interesting/useful to evaluate workloads running
        //within SoftNPU that exploit that structure.

        if inst.instance_type != "SoftNPU" {
            //TODO check this in the checker for a nicer failure mode.
            panic!("Only the SoftNPU package is supported");
        }

        if inst.parameters.len() != 2 {
            //TODO check this in the checker for a nicer failure mode.
            panic!("SoftNPU instances take exactly 2 parameters");
        }

        let parser = match self.ast.get_parser(&inst.parameters[0]) {
            Some(p) => p,
            None => {
                //TODO check this in the checker for a nicer failure mode.
                panic!("First argument to SoftNPU must be a defined parser");
            }
        };

        let control = match self.ast.get_control(&inst.parameters[1]) {
            Some(c) => c,
            None => {
                //TODO check this in the checker for a nicer failure mode.
                panic!("Second argument to SoftNPU must be a defined parser");
            }
        };

        let (table_members, table_initializers) = self.table_members(control);
        let pipeline_name = format_ident!("{}_pipeline", inst.name);

        let (parse_member, parser_initializer) = self.parse_entrypoint(parser);
        let (control_member, control_initializer) =
            self.control_entrypoint(control);

        let pipeline_impl_process_packet =
            self.pipeline_impl_process_packet(parser);

        let pipeline = quote!{
            pub struct #pipeline_name {
                #(#table_members),*,
                #parse_member,
                #control_member
            }

            impl #pipeline_name {
                pub fn new() -> Self {
                    Self {
                        #(#table_initializers),*,
                        #parser_initializer,
                        #control_initializer,
                    }
                }
            }

            impl p4rs::Pipeline for #pipeline_name {
                #pipeline_impl_process_packet
            }
        };

        self.ctx.pipelines.insert(inst.name.clone(), pipeline);

    }

    fn pipeline_impl_process_packet(
        &mut self,
        parser: &Parser,
    ) -> TokenStream {

        let parsed_type = rust_type(&parser.parameters[1].ty, false, 0);

        quote! {
            fn process_packet<'a>(
                &mut self,
                port: u8,
                pkt: &mut packet_in<'a>,
            ) -> Option<(packet_out<'a>, u8)> {

                //
                // 1. Instantiate the parser out type
                //

                let mut parsed = #parsed_type::default();

                //
                // 2. Instantiate ingress/egress metadata
                //
                let mut ingress_metadata = IngressMetadata{
                    port: {
                        let mut x = bitvec![mut u8, Msb0; 0; 8];
                        x.store(port);
                        x
                    }
                };
                let mut egress_metadata = EgressMetadata::default();

                println!("{}", "begin".green());

                //
                // 3. run the parser block
                //
                let accept = (self.parse)(pkt, &mut parsed);
                if !accept {
                    // drop the packet
                    println!("parser drop");
                    return None
                }
                println!("{}", "parser accepted".green());
                println!("{}", parsed.dump());

                //
                // 4. Calculate parsed header size
                //

                // TODO generate require a parsed_size method on header trait
                // and generate impls.
                let mut parsed_size = 0;
                if parsed.ethernet.valid {
                    parsed_size += ethernet_t::size() >> 3;
                }
                if parsed.sidecar.valid {
                    parsed_size += sidecar_t::size() >> 3;
                }
                if parsed.ipv6.valid {
                    parsed_size += ipv6_t::size() >> 3;
                }

                // 
                // 5. Run the control block
                //

                (self.control)(
                    &mut parsed,
                    &mut ingress_metadata,
                    &mut egress_metadata,
                    &self.local,
                    &self.router,
                );

                //
                // 6. Determine egress port
                //

                let port = if egress_metadata.port.is_empty() {
                    println!("{}", "no match".red());
                    println!("{}", "---".dimmed());
                    return None;
                } else {
                    egress_metadata.port.as_raw_slice()[0]
                };

                println!("{}", "control pass".green());
                println!("{}", "---".dimmed());

                //
                // 7. Create the packet output.

                let bv = parsed.to_bitvec();
                let buf = bv.as_raw_slice();
                let out = packet_out{
                    header_data: buf.to_owned(),
                    payload_data: &pkt.data[parsed_size..],
                };

                Some((out, port))

            }
        }
    }

    pub(crate) fn table_members(
        &mut self,
        control: &Control,
    ) -> (Vec<TokenStream>, Vec<TokenStream>) {
        let mut members = Vec::new();
        let mut initializers = Vec::new();
        let mut cg = crate::ControlGenerator::new(self.ast, self.hlir, self.ctx);

        // TODO below is quite repeditive with some control generator code,
        // provide better interfaces
        for table in &control.tables {
            let (_, param_types) = cg.control_parameters(control);
            let (type_tokens, _) =
                cg.generate_control_table(control, table, &param_types);
            let name = format_ident!(
                "{}_table_{}",
                control.name,
                table.name
            );
            members.push(quote! {
                #name: &#type_tokens,
            });
            let ctor = format_ident!("{}_table_{}", control.name, table.name);
            initializers.push(quote!{
                #name: #ctor()
            })
        }

        for v in &control.variables {
            if let Type::UserDefined(name) = &v.ty {
                if let Some(control_inst) = self.ast.get_control(name) {
                    let (_, param_types) = cg.control_parameters(control_inst);
                    for table in & control_inst.tables {
                        let n = table.key.len() as usize;
                        let table_type = quote! {
                            p4rs::table::Table::<#n, fn(#(#param_types),*)> 
                        };
                        let name = format_ident!("{}", v.name);
                        members.push(quote! {
                            #name: #table_type
                        });
                        let ctor = format_ident!(
                            "{}_table_{}", control_inst.name, table.name);
                        initializers.push(quote!{
                            #name: #ctor()
                        })
                    }
                }
            }
        }

        (members, initializers)
    }

    pub(crate) fn parse_entrypoint(
        &mut self,
        parser: &Parser,
    ) -> (TokenStream, TokenStream) {

        // this should never happen here, if it does it's a bug in the checker.
        let start_state = parser.get_start_state().expect(
            "parser must have start state",
        );

        let mut pg = crate::ParserGenerator::new(self.ast, self.hlir, self.ctx);
        let (sig, _) = pg.generate_state_function(parser, start_state);

        let member = quote! {
            parse: fn #sig
        };

        let initializer = format_ident!("{}_start", parser.name);
        (member, quote!{ parse: #initializer })

    }

    pub(crate) fn control_entrypoint(
        &mut self,
        control: &Control,
    ) -> (TokenStream, TokenStream) {

        let mut cg = crate::ControlGenerator::new(self.ast, self.hlir, self.ctx);
        let (sig, _) = cg.generate_control(control);

        let member = quote! {
            control: fn #sig
        };

        let initializer = format_ident!("{}_apply", control.name);
        (member, quote!{ control: #initializer })

    }
}
