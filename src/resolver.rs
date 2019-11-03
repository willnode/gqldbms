use super::{parsing, structure};
use serde_json::Value as JSONValue;
use std::collections::HashMap;

pub type ResolverArgs = Vec<(String, graphql_parser::query::Value)>;

pub struct ResolverContext<'a> {
	pub parser: &'a parsing::QueryParser,
	pub variables: &'a serde_json::Map<String, JSONValue>,
	pub fragments: &'a HashMap<std::string::String, &'a graphql_parser::query::FragmentDefinition>,
}

pub fn resolve(
	parent: &JSONValue,
	args: &ResolverArgs,
	context: &ResolverContext,
	info: &structure::StructureField,
) -> JSONValue {
	match &info.data_type.resolver {
		Some(v) => match v.kind.as_ref() {
			"CREATE" => create_resolver(&parent, &args, &context, &info),
			"UPDATE" => update_resolver(&parent, &args, &context, &info),
			"DELETE" => delete_resolver(&parent, &args, &context, &info),
			"ALL_REFERENCES" => all_references_resolver(&parent, &args, &context, &info),
			"SUBTITUTION" => subtitution_resolver(&parent, &args, &context, &info),
			"DATA" | _ => data_resolver(&parent, &args, &context, &info),
		},
		_ => data_resolver(&parent, &args, &context, &info),
	}
}

fn all_references_resolver(
	_parent: &JSONValue,
	_args: &ResolverArgs,
	context: &ResolverContext,
	info: &structure::StructureField,
) -> JSONValue {
	if !info.return_type.is_array {
		context.parser.database[&info.return_type.name[..]][0]["id"].clone()
	} else {
		json!(context.parser.database[&info.return_type.name[..]]
			.iter()
			.map(|x| x["id"].clone())
			.collect::<Vec<JSONValue>>())
	}
}

fn data_resolver(
	parent: &JSONValue,
	_args: &ResolverArgs,
	_context: &ResolverContext,
	info: &structure::StructureField,
) -> JSONValue {
	match &parent[&info.name] {
		JSONValue::Array(arr) if !info.return_type.is_array => arr[0].clone(),
		JSONValue::Null => JSONValue::Null, // resolving null is null
		n @ _ => n.clone(),
	}
}

fn subtitution_resolver(
	_parent: &JSONValue,
	_args: &ResolverArgs,
	_context: &ResolverContext,
	_info: &structure::StructureField,
) -> JSONValue {
	println!("CREATE");
	JSONValue::Null
}


fn create_resolver(
	_parent: &JSONValue,
	_args: &ResolverArgs,
	_context: &ResolverContext,
	_info: &structure::StructureField,
) -> JSONValue {
	println!("CREATE");
	JSONValue::Null
}

fn update_resolver(
	_parent: &JSONValue,
	_args: &ResolverArgs,
	_context: &ResolverContext,
	_info: &structure::StructureField,
) -> JSONValue {
	println!("UPDATE");
	JSONValue::Null
}

fn delete_resolver(
	_parent: &JSONValue,
	_args: &ResolverArgs,
	_context: &ResolverContext,
	_info: &structure::StructureField,
) -> JSONValue {
	println!("DELETE");
	JSONValue::Null
}


