// use super::{parsing, structure, resolver};
// use serde_json::Value as JSONValue;
// use std::collections::HashMap;

// pub struct CanonicalResolverContext<'a> {
// 	pub parser: &'a mut HashMap<String, parsing::QueryParser>,
// 	pub variables: &'a serde_json::Map<String, JSONValue>,
// 	pub fragments: &'a HashMap<String, &'a graphql_parser::query::FragmentDefinition>,
// }

// pub fn resolve(
// 	parent: &JSONValue,
// 	args: &resolver::ResolverArgs,
// 	context: &resolver::ResolverContext,
// 	info: &structure::StructureField,
// ) -> JSONValue {
// 	match &info.data_type.resolver {
// 		Some(v) => match v.kind.as_ref() {
// 			"CREATE" => create_resolver(&parent, &args, &context, &info),
// 			"UPDATE" => update_resolver(&parent, &args, &context, &info),
// 			"DELETE" => delete_resolver(&parent, &args, &context, &info),
// 			"ALL_REFERENCES" => all_references_resolver(&parent, &args, &context, &info),
// 			"SUBTITUTION" => subtitution_resolver(&parent, &args, &context, &info),
// 			"DATA" | _ => data_resolver(&parent, &args, &context, &info),
// 		},
// 		_ => data_resolver(&parent, &args, &context, &info),
// 	}
// }