use serde_json::Value as JSONValue;
use std::collections::HashMap;
use super::{parsing, structure};

pub type ResolverArgs = Vec<(String, graphql_parser::query::Value)>;

pub struct ResolverContext<'a> {
	pub parser: &'a parsing::QueryParser,
	pub variables: &'a serde_json::Map<String, JSONValue>,
	pub fragments: &'a HashMap<std::string::String, &'a graphql_parser::query::FragmentDefinition>
}

// pub type ResolverFn<'a> =
// 	dyn Fn(&'a JSONValue, &'a ResolverArgs, &'a ResolverContext) -> &'a JSONValue;

pub fn resolve(
	parent: &JSONValue,
	args: &ResolverArgs,
	context: &ResolverContext,
	info: &structure::StructureField,
) -> JSONValue {
	match &info.data_type.resolver {
		Some(v) if v.kind == "ALL_REFERENCES" => {
			all_references_resolver(&parent, &args, &context, &info)
		}
		_ => data_resolver(&parent, &args, &context, &info),
	}
}

fn all_references_resolver<'a>(
	_parent: &'a JSONValue,
	_args: &ResolverArgs,
	context: &ResolverContext,
	info: &structure::StructureField,
) -> JSONValue {
	if !info.return_type.is_array {
		context.parser.database[&info.return_type.name[..]][0]["id"].clone()
	} else {
		json!(context.parser.database[&info.return_type.name[..]].iter().map(|x| x["id"].clone()).collect::<Vec<JSONValue>>())
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
