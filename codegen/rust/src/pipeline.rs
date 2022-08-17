use crate::{rust_type, type_size, Context};
use p4::ast::{Control, MatchKind, PackageInstance, Parser, Table, Type, AST};
use p4::hlir::Hlir;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub(crate) struct PipelineGenerator<'a> {
    ast: &'a AST,
    ctx: &'a mut Context,
    hlir: &'a Hlir,
}

impl<'a> PipelineGenerator<'a> {
    pub(crate) fn new(
        ast: &'a AST,
        hlir: &'a Hlir,
        ctx: &'a mut Context,
    ) -> Self {
        Self { ast, hlir, ctx }
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
            self.pipeline_impl_process_packet(parser, control);

        let add_table_entry_method = self.add_table_entry_method(control);

        let remove_table_entry_method = self.remove_table_entry_method(control);

        let table_modifiers = self.table_modifiers(control);

        let pipeline = quote! {
            pub struct #pipeline_name {
                #(#table_members),*,
                #parse_member,
                #control_member
            }

            impl #pipeline_name {
                pub fn new() -> Self {
                    usdt::register_probes().unwrap();
                    Self {
                        #(#table_initializers),*,
                        #parser_initializer,
                        #control_initializer,
                    }
                }
                #table_modifiers
            }

            impl p4rs::Pipeline for #pipeline_name {
                #pipeline_impl_process_packet
                #add_table_entry_method
                #remove_table_entry_method
            }

            unsafe impl Send for #pipeline_name { }
        };

        self.ctx.pipelines.insert(inst.name.clone(), pipeline);
    }

    fn pipeline_impl_process_packet(
        &mut self,
        parser: &Parser,
        control: &Control,
    ) -> TokenStream {
        let parsed_type = rust_type(&parser.parameters[1].ty);

        // determine table arguments
        let tables = control.tables(self.ast);
        let mut tbl_args = Vec::new();
        for (c, t) in tables {
            let name = format_ident!("{}_table_{}", c.name, t.name);
            tbl_args.push(quote! {
                &self.#name
            });
        }

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
                    dtrace_provider::parser_dropped!(||());
                    println!("parser drop");
                    return None
                }
                println!("{}", "parser accepted".green()); //XXX
                let dump = parsed.dump();
                println!("{}", "<<<".dimmed());
                println!("{}", &dump); //XXX
                dtrace_provider::parser_accepted!(||(&dump));

                //
                // 4. Calculate parsed header size
                //

                let parsed_size = parsed.valid_header_size() >> 3;

                //
                // 5. Run the control block
                //

                (self.control)(
                    &mut parsed,
                    &mut ingress_metadata,
                    &mut egress_metadata,
                    #(#tbl_args),*
                );

                //
                // 6. Determine egress port
                //

                let port = if egress_metadata.port.is_empty()
                    || egress_metadata.drop {
                    dtrace_provider::control_dropped!(||(&dump));
                    println!("{}", "no match".red());
                    println!("{}", "---".dimmed());
                    return None;
                } else {
                    egress_metadata.port.as_raw_slice()[0]
                };

                let dump = parsed.dump();
                println!("{}", ">>>".dimmed());
                println!("{}", &dump); //XXX

                dtrace_provider::control_accepted!(||(&dump));
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
        let mut cg =
            crate::ControlGenerator::new(self.ast, self.hlir, self.ctx);

        let tables = control.tables(self.ast);
        for (control, table) in tables {
            let (_, param_types) = cg.control_parameters(control);
            let n = table.key.len() as usize;
            let table_type = quote! {
                p4rs::table::Table::<
                    #n,
                    std::sync::Arc<dyn Fn(#(#param_types),*)>
                    >
            };
            let name = format_ident!("{}_table_{}", control.name, table.name);
            members.push(quote! {
                pub #name: #table_type
            });
            let ctor = format_ident!("{}_table_{}", control.name, table.name);
            initializers.push(quote! {
                #name: #ctor()
            })
        }

        (members, initializers)
    }

    fn add_table_entry_method(&mut self, control: &Control) -> TokenStream {
        let mut body = TokenStream::new();

        let tables = control.tables(self.ast);
        for (i, (_control, table)) in tables.iter().enumerate() {
            let i = i as u32;
            let call = format_ident!("add_{}_table_entry", table.name);
            body.extend(quote! {
                #i => self.#call(
                    action_id,
                    keyset_data,
                    parameter_data,
                ),
            });
        }

        body.extend(quote! {
            x => println!("add table entry: unknown table id {}, ignoring", x),
        });

        quote! {
            fn add_table_entry(
                &mut self,
                table_id: u32,
                action_id: u32,
                keyset_data: &[u8],
                parameter_data: &[u8],
            ) {
                match table_id {
                    #body
                }
            }
        }
    }

    fn remove_table_entry_method(&mut self, control: &Control) -> TokenStream {
        let mut body = TokenStream::new();

        let tables = control.tables(self.ast);
        for (i, (_control, table)) in tables.iter().enumerate() {
            let i = i as u32;
            //TODO probably a conflict with the same table name in multiple control
            //blocks
            let call = format_ident!("remove_{}_table_entry", table.name);
            body.extend(quote! {
                #i => self.#call(keyset_data),
            });
        }

        body.extend(quote!{
            x => println!("remove table entry: unknown table id {}, ignoring", x),
        });

        quote! {
            fn remove_table_entry(
                &mut self,
                table_id: u32,
                keyset_data: &[u8],
            ) {
                match table_id {
                    #body
                }
            }
        }
    }

    fn table_modifiers(&mut self, control: &Control) -> TokenStream {
        let mut tokens = TokenStream::new();
        let tables = control.tables(self.ast);
        for (control, table) in tables {
            tokens.extend(self.add_table_entry_function(table, control));
            tokens.extend(self.remove_table_entry_function(table, control));
        }

        tokens
    }

    fn table_entry_keys(&mut self, table: &Table) -> TokenStream {
        let mut keys = TokenStream::new();
        let mut offset: usize = 0;
        for (lval, match_kind) in &table.key {
            let name_info =
                self.hlir.lvalue_decls.get(lval).unwrap_or_else(|| {
                    panic!("declaration info for {:#?}", lval,)
                });
            let sz = type_size(&name_info.ty, self.ast) >> 3;
            match match_kind {
                MatchKind::Exact => keys.extend(quote! {
                    p4rs::extract_exact_key(
                        keyset_data,
                        #offset,
                        #sz,
                    )
                }),
                MatchKind::Ternary => keys.extend(quote! {
                    p4rs::extract_ternary_key(
                        keyset_data,
                        #offset,
                        #sz,
                    )
                }),
                MatchKind::LongestPrefixMatch => keys.extend(quote! {
                    p4rs::extract_lpm_key(
                        keyset_data,
                        #offset,
                        #sz,
                    )
                }),
                MatchKind::Range => keys.extend(quote! {
                    p4rs::extract_range_key(
                        keyset_data,
                        #offset,
                        #sz,
                    )
                }),
            }
            offset += sz;
        }

        keys
    }

    fn add_table_entry_function(
        &mut self,
        table: &Table,
        control: &Control,
    ) -> TokenStream {
        let keys = self.table_entry_keys(table);

        let mut action_match_body = TokenStream::new();
        for (i, action) in table.actions.iter().enumerate() {
            let i = i as u32;
            let call = format_ident!("{}_action_{}", table.name, action);
            let n = table.key.len();
            //XXX hack
            if action == "NoAction" {
                continue;
            }
            let a = control.get_action(action).unwrap_or_else(|| {
                panic!("control {} must have action {}", control.name, action,)
            });
            let mut parameter_tokens = Vec::new();
            let mut parameter_refs = Vec::new();
            let mut offset: usize = 0;
            for p in &a.parameters {
                let pname = format_ident!("{}", p.name);
                match &p.ty {
                    Type::Bool => {
                        parameter_tokens.push(quote! {
                            let #pname = p4rs::extract_bool_action_parameter(
                                parameter_data,
                                #offset,
                            );
                        });
                        offset += 1;
                    }
                    Type::Error => {
                        todo!();
                    }
                    Type::Bit(n) => {
                        parameter_tokens.push(quote! {
                            let #pname = p4rs::extract_bit_action_parameter(
                                parameter_data,
                                #offset,
                                #n,
                            );
                        });
                        parameter_refs.push(quote! { #pname.clone() });
                        offset += 1;
                    }
                    Type::Varbit(_n) => {
                        todo!();
                    }
                    Type::Int(_n) => {
                        todo!();
                    }
                    Type::String => {
                        todo!();
                    }
                    Type::UserDefined(_s) => {
                        todo!();
                    }
                    Type::ExternFunction => {
                        todo!();
                    }
                    Type::Table => {
                        todo!();
                    }
                    Type::Void => {
                        todo!();
                    }
                }
            }
            let mut control_params = Vec::new();
            let mut control_param_types = Vec::new();
            let mut action_params = Vec::new();
            let mut action_param_types = Vec::new();
            for p in &control.parameters {
                let name = format_ident!("{}", p.name);
                control_params.push(quote! { #name });
                let ty = rust_type(&p.ty);
                control_param_types.push(quote! { &mut #ty });
            }
            for p in &a.parameters {
                let name = format_ident!("{}", p.name);
                action_params.push(quote! { #name });
                let ty = rust_type(&p.ty);
                action_param_types.push(quote! { #ty });
            }
            //XXX let tname = format_ident!("{}", table.name);
            let tname = format_ident!("{}_table_{}", control.name, table.name);
            action_match_body.extend(quote! {
                #i => {
                    #(#parameter_tokens)*
                    let action: std::sync::Arc<dyn Fn(
                        #(#control_param_types),*
                    )>
                    = std::sync::Arc::new(move |
                        #(#control_params),*
                    | {
                        #call(
                            #(#control_params),*,
                            #(#parameter_refs),*
                        )
                    });
                    self.#tname
                        .entries
                        .insert(p4rs::table::TableEntry::<
                            #n,
                            std::sync::Arc<dyn Fn(
                                #(#control_param_types),*
                            )>,
                        > {
                            key,
                            priority: 0, //TODO
                            name: "your name here".into(), //TODO
                            action,
                        });
                }
            });
        }
        let name = &control.name;
        action_match_body.extend(quote! {
            x => panic!("unknown {} action id {}", #name, x),
        });

        let name = format_ident!("add_{}_table_entry", table.name);
        quote! {
            // lifetime is due to
            // https://github.com/rust-lang/rust/issues/96771#issuecomment-1119886703
            pub fn #name<'a>(
                &mut self,
                action_id: u32,
                keyset_data: &'a [u8],
                parameter_data: &'a [u8],
            ) {

                let key = [#keys];

                match action_id {
                    #action_match_body
                }

            }
        }
    }

    fn remove_table_entry_function(
        &mut self,
        table: &Table,
        control: &Control,
    ) -> TokenStream {
        let keys = self.table_entry_keys(table);
        let n = table.key.len();

        //let tname = format_ident!("{}", table.name);
        let tname = format_ident!("{}_table_{}", control.name, table.name);
        let name = format_ident!("remove_{}_table_entry", table.name);

        let mut control_params = Vec::new();
        let mut control_param_types = Vec::new();
        for p in &control.parameters {
            let name = format_ident!("{}", p.name);
            control_params.push(quote! { #name });
            let ty = rust_type(&p.ty);
            control_param_types.push(quote! { &mut #ty });
        }

        quote! {
            // lifetime is due to
            // https://github.com/rust-lang/rust/issues/96771#issuecomment-1119886703
            pub fn #name<'a>(
                &mut self,
                keyset_data: &'a [u8],
            ) {

                let key = [#keys];

                let action: std::sync::Arc<dyn Fn(
                    #(#control_param_types),*
                )>
                = std::sync::Arc::new(move |
                    #(#control_params),*
                | { });

                self.#tname
                    .entries
                    .remove(
                        &p4rs::table::TableEntry::<
                            #n,
                            std::sync::Arc<dyn Fn(
                                #(#control_param_types),*
                            )>,
                        > {
                            key,
                            priority: 0, //TODO
                            name: "your name here".into(), //TODO
                            action,
                        }
                    );

            }
        }
    }

    pub(crate) fn parse_entrypoint(
        &mut self,
        parser: &Parser,
    ) -> (TokenStream, TokenStream) {
        // this should never happen here, if it does it's a bug in the checker.
        let start_state = parser
            .get_start_state()
            .expect("parser must have start state");

        let mut pg = crate::ParserGenerator::new(self.ast, self.hlir, self.ctx);
        let (sig, _) = pg.generate_state_function(parser, start_state);

        let member = quote! {
            pub parse: fn #sig
        };

        let initializer = format_ident!("{}_start", parser.name);
        (member, quote! { parse: #initializer })
    }

    pub(crate) fn control_entrypoint(
        &mut self,
        control: &Control,
    ) -> (TokenStream, TokenStream) {
        let mut cg =
            crate::ControlGenerator::new(self.ast, self.hlir, self.ctx);
        let (sig, _) = cg.generate_control(control);

        let member = quote! {
            pub control: fn #sig
        };

        let initializer = format_ident!("{}_apply", control.name);
        (member, quote! { control: #initializer })
    }
}
