//#![allow(dead_code)]
//#![allow(incomplete_features)]
//#![allow(unused_imports)]

//mod hub;
//mod router;

// TODO generateion of _main_pipeline_create symbol uniformly for all
// pipelines means we can only use p4_macro::use_p4 once per crate :/

//#[cfg(test)]
//mod disag_router;
//#[cfg(test)]
//mod dynamic_router;
#[cfg(test)]
mod mac_rewrite;
#[cfg(test)]
mod softnpu;
//#[cfg(test)]
//mod headers;
