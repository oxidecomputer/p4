// Copyright 2022 Oxide Computer Company

use crate::{
    qualified_table_function_name, qualified_table_name, rust_type,
    type_size_bytes, Context, Settings,
};
use p4::ast::{
    Control, Direction, MatchKind, PackageInstance, Parser, Table, Type, AST,
};
use p4::hlir::Hlir;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub(crate) struct PipelineGenerator<'a> {
    ast: &'a AST,
    ctx: &'a mut Context,
    hlir: &'a Hlir,
    settings: &'a Settings,
}

impl<'a> PipelineGenerator<'a> {
    pub(crate) fn new(
        ast: &'a AST,
        hlir: &'a Hlir,
        ctx: &'a mut Context,
        settings: &'a Settings,
    ) -> Self {
        Self {
            ast,
            hlir,
            ctx,
            settings,
        }
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

        if inst.parameters.len() != 3 {
            //TODO check this in the checker for a nicer failure mode.
            panic!("SoftNPU instances take exactly 3 parameters");
        }

        let parser = match self.ast.get_parser(&inst.parameters[0]) {
            Some(p) => p,
            None => {
                //TODO check this in the checker for a nicer failure mode.
                panic!("First argument to SoftNPU must be a parser");
            }
        };

        let ingress = match self.ast.get_control(&inst.parameters[1]) {
            Some(c) => c,
            None => {
                //TODO check this in the checker for a nicer failure mode.
                panic!("Second argument to SoftNPU must be a control block");
            }
        };

        let egress = match self.ast.get_control(&inst.parameters[2]) {
            Some(c) => c,
            None => {
                //TODO check this in the checker for a nicer failure mode.
                panic!("Third argument to SoftNPU must be a control block");
            }
        };

        let pipeline_name = format_ident!("{}_pipeline", inst.name);

        //
        // get table members and initializers from both the ingress and egress
        // controllers.
        //

        let (mut table_members, mut table_initializers) =
            self.table_members(ingress);

        let (egress_table_members, egress_table_initializers) =
            self.table_members(egress);

        table_members.extend_from_slice(&egress_table_members);
        table_initializers.extend_from_slice(&egress_table_initializers);

        //
        // parser, ingress and egress function members
        //

        let (parse_member, parser_initializer) = self.parse_entrypoint(parser);

        let (ingress_member, ingress_initializer) =
            self.control_entrypoint("ingress", ingress);

        let (egress_member, egress_initializer) =
            self.control_entrypoint("egress", egress);

        let pipeline_impl_process_packet =
            self.pipeline_impl_process_packet(parser, ingress, egress);

        let add_table_entry_method =
            self.add_table_entry_method(ingress, egress);
        let remove_table_entry_method =
            self.remove_table_entry_method(ingress, egress);
        let get_table_entries_method =
            self.get_table_entries_method(ingress, egress);
        let get_table_ids_method = self.get_table_ids_method(ingress, egress);

        let table_modifiers = self.table_modifiers(ingress, egress);

        let c_create_fn =
            format_ident!("_{}_pipeline_create", self.settings.pipeline_name);

        let pipeline = quote! {
            pub struct #pipeline_name {
                #(#table_members),*,
                #parse_member,
                #ingress_member,
                #egress_member,
                radix: u16,
            }

            impl #pipeline_name {
                pub fn new(radix: u16) -> Self {
                    usdt::register_probes().unwrap();
                    Self {
                        #(#table_initializers),*,
                        #parser_initializer,
                        #ingress_initializer,
                        #egress_initializer,
                        radix,
                    }
                }
                #table_modifiers
            }

            impl p4rs::Pipeline for #pipeline_name {
                #pipeline_impl_process_packet
                #add_table_entry_method
                #remove_table_entry_method
                #get_table_entries_method
                #get_table_ids_method
            }

            unsafe impl Send for #pipeline_name { }

            #[no_mangle]
            pub extern "C" fn #c_create_fn(radix: u16)
            -> *mut dyn p4rs::Pipeline{
                let pipeline = main_pipeline::new(radix);
                let boxpipe: Box<dyn p4rs::Pipeline> = Box::new(pipeline);
                Box::into_raw(boxpipe)
            }
        };

        self.ctx.pipelines.insert(inst.name.clone(), pipeline);
    }

    fn pipeline_impl_process_packet(
        &mut self,
        parser: &Parser,
        ingress: &Control,
        egress: &Control,
    ) -> TokenStream {
        let parsed_type = rust_type(&parser.parameters[1].ty);
        // determine table arguments
        let ingress_tables = ingress.tables(self.ast);
        //TODO(dry)
        let mut ingress_tbl_args = Vec::new();
        for (cs, t) in ingress_tables {
            let qtfn = qualified_table_function_name(Some(ingress), &cs, t);
            let name = format_ident!("{}", qtfn);
            ingress_tbl_args.push(quote! {
                &self.#name
            });
        }
        let egress_tables = egress.tables(self.ast);
        let mut egress_tbl_args = Vec::new();
        for (cs, t) in egress_tables {
            let qtfn = qualified_table_function_name(Some(egress), &cs, t);
            let name = format_ident!("{}", qtfn);
            egress_tbl_args.push(quote! {
                &self.#name
            });
        }

        quote! {
            fn process_packet<'a>(
                &mut self,
                port: u16,
                pkt: &mut packet_in<'a>,
            ) -> Vec<(packet_out<'a>, u16)> {
                //
                // Instantiate the parser out type
                //

                let mut parsed = #parsed_type::default();

                //
                // Instantiate ingress/egress metadata
                //

                let mut ingress_metadata = ingress_metadata_t{
                    port: {
                        let mut x = bitvec![mut u8, Msb0; 0; 16];
                        x.store_le(port);
                        x
                    },
                    ..Default::default()
                };
                let mut egress_metadata = egress_metadata_t::default();

                //
                // Run the parser block
                //

                let accept = (self.parse)(pkt, &mut parsed, &mut ingress_metadata);
                if !accept {
                    // drop the packet
                    softnpu_provider::parser_dropped!(||());
                    return Vec::new();
                }
                let dump = parsed.dump();
                softnpu_provider::parser_accepted!(||(&dump));

                //
                // Calculate parsed header size
                //

                let parsed_size = parsed.valid_header_size() >> 3;

                //
                // Run the ingress block
                //

                (self.ingress)(
                    &mut parsed,
                    &mut ingress_metadata,
                    &mut egress_metadata,
                    #(#ingress_tbl_args),*
                );

                //
                // Determine egress ports
                //

                let ports = if egress_metadata.broadcast {
                    let mut ports = Vec::new();
                    for p in 0..self.radix {
                        if p == port {
                            continue;
                        }
                        ports.push(p);
                    }
                    ports
                } else {
                    if egress_metadata.port.is_empty() || egress_metadata.drop {
                        Vec::new()
                    } else {
                        vec![egress_metadata.port.load_le()]
                    }
                };

                let dump = parsed.dump();

                if ports.is_empty() {
                    softnpu_provider::ingress_dropped!(||(&dump));
                    return Vec::new();
                }

                softnpu_provider::ingress_accepted!(||(&dump));

                //
                // Run output of ingress block through egress block on each
                // egress port.
                //
                let mut result = Vec::new();
                for eport in ports {

                    let mut egm = egress_metadata.clone();
                    let mut parsed_ = parsed.clone();

                    //
                    // Run the egress block
                    //

                    egm.port = {
                        let mut x = bitvec![mut u8, Msb0; 0; 16];
                        x.store_le(eport);
                        x
                    };

                    (self.egress)(
                        &mut parsed_,
                        &mut ingress_metadata,
                        &mut egm,
                        #(#egress_tbl_args),*
                    );

                    if egm.drop {
                        continue;
                    }

                    //
                    // Create the packet output.
                    //

                    let bv = parsed_.to_bitvec();
                    let buf = bv.as_raw_slice();
                    let out = packet_out{
                        header_data: buf.to_owned(),
                        payload_data: &pkt.data[parsed_size..],
                    };
                    result.push((out, eport))

                }
                result
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
        for (cs, table) in tables {
            let table_control = cs.last().unwrap().1;
            let qtn = qualified_table_function_name(Some(control), &cs, table);
            let fqtn = qualified_table_function_name(Some(control), &cs, table);
            let (_, mut param_types) = cg.control_parameters(table_control);

            for var in &table_control.variables {
                if let Type::UserDefined(typename) = &var.ty {
                    if self.ast.get_extern(typename).is_some() {
                        let extern_type = format_ident!("{}", typename);
                        param_types.push(quote! {
                            &p4rs::externs::#extern_type
                        })
                    }
                }
            }

            let n = table.key.len();
            let table_type = quote! {
                p4rs::table::Table::<
                    #n,
                    std::sync::Arc<dyn Fn(#(#param_types),*)>
                    >
            };
            let qtn = format_ident!("{}", qtn);
            let fqtn = format_ident!("{}", fqtn);
            members.push(quote! {
                pub #qtn: #table_type
            });
            initializers.push(quote! {
                #qtn: #fqtn()
            })
        }

        (members, initializers)
    }

    fn add_table_entry_method(
        &mut self,
        ingress: &Control,
        egress: &Control,
    ) -> TokenStream {
        let mut body = TokenStream::new();

        for control in &[ingress, egress] {
            let tables = control.tables(self.ast);
            for (cs, table) in tables.iter() {
                let qtn = qualified_table_name(Some(control), cs, table);
                let qtfn =
                    qualified_table_function_name(Some(control), cs, table);
                let call = format_ident!("add_{}_entry", qtfn);
                body.extend(quote! {
                    #qtn => self.#call(
                        action_id,
                        keyset_data,
                        parameter_data,
                    ),
                });
            }
        }

        body.extend(quote! {
            x => println!("add table entry: unknown table id {}, ignoring", x),
        });

        quote! {
            fn add_table_entry(
                &mut self,
                table_id: &str,
                action_id: &str,
                keyset_data: &[u8],
                parameter_data: &[u8],
            ) {
                match table_id {
                    #body
                }
            }
        }
    }

    fn remove_table_entry_method(
        &mut self,
        ingress: &Control,
        egress: &Control,
    ) -> TokenStream {
        let mut body = TokenStream::new();

        for control in &[ingress, egress] {
            let tables = control.tables(self.ast);
            for (cs, table) in tables.iter() {
                let qtn = qualified_table_name(Some(control), cs, table);
                let qftn =
                    qualified_table_function_name(Some(control), cs, table);
                let call = format_ident!("remove_{}_entry", qftn);
                body.extend(quote! {
                    #qtn => self.#call(keyset_data),
                });
            }
        }

        body.extend(quote!{
            x => println!("remove table entry: unknown table id {}, ignoring", x),
        });

        quote! {
            fn remove_table_entry(
                &mut self,
                table_id: &str,
                keyset_data: &[u8],
            ) {
                match table_id {
                    #body
                }
            }
        }
    }

    fn get_table_ids_method(
        &mut self,
        ingress: &Control,
        egress: &Control,
    ) -> TokenStream {
        let mut names = Vec::new();

        for control in &[ingress, egress] {
            let tables = control.tables(self.ast);
            for (cs, table) in &tables {
                names.push(qualified_table_name(Some(control), cs, table));
            }
        }
        quote! {
            fn get_table_ids(&self) -> Vec<&str> {
                vec![#(#names),*]
            }
        }
    }

    fn get_table_entries_method(
        &mut self,
        ingress: &Control,
        egress: &Control,
    ) -> TokenStream {
        let mut body = TokenStream::new();

        for control in &[ingress, egress] {
            let tables = control.tables(self.ast);
            for (cs, table) in tables.iter() {
                let qtn = qualified_table_name(Some(control), cs, table);
                let qtfn =
                    qualified_table_function_name(Some(control), cs, table);
                let call = format_ident!("get_{}_entries", qtfn);
                body.extend(quote! {
                    #qtn => Some(self.#call()),
                });
            }
        }

        body.extend(quote! {
            x => None,
        });

        quote! {
            fn get_table_entries(
                &self,
                table_id: &str,
            ) -> Option<Vec<p4rs::TableEntry>> {
                match table_id {
                    #body
                }
            }
        }
    }

    fn table_modifiers(
        &mut self,
        ingress: &Control,
        egress: &Control,
    ) -> TokenStream {
        let mut tokens = TokenStream::new();
        self.table_modifiers_for_control(&mut tokens, ingress);
        self.table_modifiers_for_control(&mut tokens, egress);
        tokens
    }

    fn table_modifiers_for_control(
        &mut self,
        tokens: &mut TokenStream,
        control: &Control,
    ) {
        let tables = control.tables(self.ast);
        for (cs, table) in tables {
            let table_control = cs.last().unwrap().1;
            let qtfn = qualified_table_function_name(Some(control), &cs, table);
            tokens.extend(self.add_table_entry_function(
                table,
                table_control,
                &qtfn,
            ));
            tokens.extend(self.remove_table_entry_function(
                table,
                table_control,
                &qtfn,
            ));
            tokens.extend(self.get_table_entries_function(
                table,
                table_control,
                &qtfn,
            ));
        }
    }

    fn table_entry_keys(&mut self, table: &Table) -> Vec<TokenStream> {
        let mut keys = Vec::new();
        let mut offset: usize = 0;
        for (lval, match_kind) in &table.key {
            let name_info =
                self.hlir.lvalue_decls.get(lval).unwrap_or_else(|| {
                    panic!("declaration info for {:#?}", lval,)
                });
            let sz = type_size_bytes(&name_info.ty, self.ast);
            match match_kind {
                MatchKind::Exact => keys.push(quote! {
                    p4rs::extract_exact_key(
                        keyset_data,
                        #offset,
                        #sz,
                    )
                }),
                MatchKind::Ternary => keys.push(quote! {
                    p4rs::extract_ternary_key(
                        keyset_data,
                        #offset,
                        #sz,
                    )
                }),
                MatchKind::LongestPrefixMatch => keys.push(quote! {
                    p4rs::extract_lpm_key(
                        keyset_data,
                        #offset,
                        #sz,
                    )
                }),
                MatchKind::Range => keys.push(quote! {
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
        qtfn: &str,
    ) -> TokenStream {
        let keys = self.table_entry_keys(table);

        let mut action_match_body = TokenStream::new();
        for action in table.actions.iter() {
            let call =
                format_ident!("{}_action_{}", control.name, &action.name);
            let n = table.key.len();
            //XXX hack
            if &action.name == "NoAction" {
                continue;
            }
            let a = control.get_action(&action.name).unwrap_or_else(|| {
                panic!(
                    "control {} must have action {}",
                    control.name, &action.name,
                )
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
                    Type::State => {
                        todo!();
                    }
                    Type::Action => {
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
                        offset += n >> 3;
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
                    Type::HeaderMethod => {
                        todo!();
                    }
                    Type::Table => {
                        todo!();
                    }
                    Type::Void => {
                        todo!();
                    }
                    Type::List(_) => {
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
                match p.direction {
                    Direction::Out | Direction::InOut => {
                        control_param_types.push(quote! { &mut #ty });
                    }
                    _ => {
                        control_param_types.push(quote! { &#ty });
                    }
                }
            }

            for p in &a.parameters {
                let name = format_ident!("{}", p.name);
                action_params.push(quote! { #name });
                let ty = rust_type(&p.ty);
                action_param_types.push(quote! { #ty });
            }

            for var in &control.variables {
                let name = format_ident!("{}", var.name);
                if let Type::UserDefined(typename) = &var.ty {
                    if self.ast.get_extern(typename).is_some() {
                        control_params.push(quote! { #name });
                        let extern_type = format_ident!("{}", typename);
                        control_param_types.push(quote! {
                            &p4rs::externs::#extern_type
                        });
                    }
                }
            }

            let aname = &action.name;
            let tname = format_ident!("{}", qtfn);
            action_match_body.extend(quote! {
                #aname => {
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
                            action_id: #aname.to_owned(),
                            parameter_data: parameter_data.to_owned(),
                        });
                }
            });
        }
        let name = &control.name;
        action_match_body.extend(quote! {
            x => panic!("unknown {} action id {}", #name, x),
        });

        let name = format_ident!("add_{}_entry", qtfn);
        quote! {
            // lifetime is due to
            // https://github.com/rust-lang/rust/issues/96771#issuecomment-1119886703
            pub fn #name<'a>(
                &mut self,
                action_id: &str,
                keyset_data: &'a [u8],
                parameter_data: &'a [u8],
            ) {

                let key = [#(#keys),*];

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
        qtfn: &str,
    ) -> TokenStream {
        let keys = self.table_entry_keys(table);
        let n = table.key.len();

        let tname = format_ident!("{}", qtfn);
        let name = format_ident!("remove_{}_entry", qtfn);

        let mut control_params = Vec::new();
        let mut control_param_types = Vec::new();
        for p in &control.parameters {
            let name = format_ident!("{}", p.name);
            control_params.push(quote! { #name });
            let ty = rust_type(&p.ty);
            match p.direction {
                Direction::Out | Direction::InOut => {
                    control_param_types.push(quote! { &mut #ty });
                }
                _ => {
                    control_param_types.push(quote! { &#ty });
                }
            }
        }

        for var in &control.variables {
            let name = format_ident!("{}", var.name);
            if let Type::UserDefined(typename) = &var.ty {
                if self.ast.get_extern(typename).is_some() {
                    control_params.push(quote! { #name });
                    let extern_type = format_ident!("{}", typename);
                    control_param_types.push(quote! {
                        &p4rs::externs::#extern_type
                    });
                }
            }
        }

        quote! {
            // lifetime is due to
            // https://github.com/rust-lang/rust/issues/96771#issuecomment-1119886703
            pub fn #name<'a>(
                &mut self,
                keyset_data: &'a [u8],
            ) {

                let key = [#(#keys),*];

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
                            action_id: String::new(),
                            parameter_data: Vec::new(),
                        }
                    );

            }
        }
    }

    fn get_table_entries_function(
        &mut self,
        _table: &Table,
        _control: &Control,
        qtfn: &str,
    ) -> TokenStream {
        let name = format_ident!("get_{}_entries", qtfn);
        let tname = format_ident!("{}", qtfn);

        quote! {
            pub fn #name(&self) -> Vec<p4rs::TableEntry> {
                let mut result = Vec::new();

                for e in &self.#tname.entries{

                    let mut keyset_data = Vec::new();
                    for k in &e.key {
                        //TODO this is broken, to_bytes can squash N byte
                        //objects into smaller than N bytes which violates
                        //expectations of consumers. For example if this is a
                        //16-bit integer with a value of 47 it will get squashed
                        //down into 8-bits.
                        keyset_data.extend_from_slice(&k.to_bytes());
                    }

                    let x = p4rs::TableEntry{
                        action_id: e.action_id.clone(),
                        keyset_data,
                        parameter_data: e.parameter_data.clone(),
                    };

                    result.push(x);

                }

                result
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
        name: &str,
        control: &Control,
    ) -> (TokenStream, TokenStream) {
        let mut cg =
            crate::ControlGenerator::new(self.ast, self.hlir, self.ctx);
        let (sig, _) = cg.generate_control(control);

        let name = format_ident!("{}", name);

        let member = quote! {
            pub #name: fn #sig
        };

        let initializer = format_ident!("{}_apply", control.name);
        (member, quote! { #name: #initializer })
    }
}
