use super::{parsing, utility};
use graphql_parser::parse_schema;
use graphql_parser::schema::*;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;

pub fn get_field_type(t: &Type) -> SchemaFieldReturnType {
	let mut r = SchemaFieldReturnType {
		is_array: false,
		is_array_non_nullable: false,
		name_type: String::new(),
		is_type_non_nullable: false,
	};
	let mut t = t.clone();
	loop {
		t = match &t {
			Type::ListType(tt) => {
				r.is_array = true;
				r.is_array_non_nullable = r.is_type_non_nullable;
				r.is_type_non_nullable = false;
				tt.deref().clone()
			}
			Type::NonNullType(tt) => {
				r.is_type_non_nullable = true;
				tt.deref().clone()
			}
			Type::NamedType(name) => {
				r.name_type.push_str(name);
				break;
			}
		};
	}
	r
}

pub fn get_data_type(t: &SchemaFieldReturnType, d: &str) -> SchemaFieldDataType {
	let mut data_type = match &t.name_type[..] {
		"ID" => "string",
		"Int" => "i32",
		"Float" => "f64",
		"String" => "string",
		"Boolean" => "bool",
		n @ _ => n,
	};
	if d.contains("@type as i32;") {
		data_type = "i32";
	}
	SchemaFieldDataType {
		is_array: t.is_array,
		is_indexed: t.name_type == "ID",
		is_unique: t.name_type == "ID",
		name_type: data_type.to_owned(),
	}
}

#[derive(Clone, Debug)]
pub struct SchemaField {
	pub name: String,
	pub description: String,
	pub data_type: SchemaFieldDataType,
	pub return_type: SchemaFieldReturnType,
}

impl SchemaField {
	pub fn new(name: &str, data_type: &str, array: bool) -> SchemaField {
		let rtype = SchemaFieldReturnType {
			is_array: array,
			is_array_non_nullable: true,
			name_type: data_type.to_owned(),
			is_type_non_nullable: name == "id",
		};
		SchemaField {
			name: name.to_owned(),
			description: "".to_owned(),
			data_type: get_data_type(&rtype, ""),
			return_type: rtype,
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchemaFieldDataType {
	pub name_type: String,
	pub is_array: bool,
	pub is_unique: bool,
	pub is_indexed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchemaFieldReturnType {
	pub is_array: bool,
	pub is_array_non_nullable: bool,
	pub name_type: String,
	pub is_type_non_nullable: bool,
}

pub type SchemaClasses = HashMap<String, SchemaType>;
pub type SchemaFields = HashMap<String, SchemaField>;

fn read_schema(path: &str) -> Document {
	let data = utility::read_db_file(path);
	parse_schema(&data)
		.expect(&format!("File `database/{}` is not valid GraphQL schema!", path)[..])
}

fn traverse_object(object: &ObjectType) -> SchemaFields {
	let mut fields = HashMap::new();
	for field in &object.fields {
		let return_type = get_field_type(&field.field_type);
		let description = match &field.description {
			Some(v) => v.clone(),
			_ => String::new(),
		};
		let data_type = get_data_type(&return_type, &description[..]);
		fields.insert(
			field.name.clone(),
			SchemaField {
				name: field.name.clone(),
				description: description,
				return_type: return_type,
				data_type: data_type,
			},
		);
	}
	fields
}

#[derive(Clone, Debug)]
pub enum SchemaType {
	Object(SchemaFields),
	Enum(HashSet<String>),
}

pub fn traverse_schema(file: &str) -> SchemaClasses {
	let doc = read_schema(file);
	let mut hashes = HashMap::new();
	for def in doc.definitions {
		match &def {
			Definition::TypeDefinition(typedef) => match &typedef {
				TypeDefinition::Scalar(_) => {}
				TypeDefinition::Object(object) => {
					hashes.insert(
						object.name.clone(),
						SchemaType::Object(traverse_object(&object)),
					);
				}
				TypeDefinition::Enum(enu) => {
					let enus = enu
						.values
						.iter()
						.map(|x| x.name.clone())
						.collect::<HashSet<String>>();
					hashes.insert(enu.name.clone(), SchemaType::Enum(enus));
				}
				_ => {}
			},
			_ => {}
		}
	}
	hashes
}

pub struct InstropectionParser {
	pub schema: SchemaClasses,
	pub database: parsing::DatabaseIndex,
}

pub fn build_schema_instropection(db: &str) -> InstropectionParser {
	let mut fields = Vec::new();
	let mut types = Vec::new();
	let mut enums = Vec::new();
	let doc = read_schema(&format!("{}/schema.gql", db)[..]);

	for def in &doc.definitions {
		match &def {
			Definition::TypeDefinition(typedef) => match &typedef {
				TypeDefinition::Scalar(object) => {
					types.push(json!({
						"id": object.name.clone(),
						"name": object.name.clone(),
						"kind": "SCALAR",
						"description": &object.description,
						"interfaces": []
					}));
				}
				TypeDefinition::Object(object) => {
					let mut subfields = Vec::new();
					for field in &object.fields {
						fields.push(json!({
							"id": format!("{}.{}", object.name, field.name),
							"name": field.name,
							"description": field.description,
							"isDeprecated": false,
							"args": [],
							"type": get_field_type(&field.field_type).name_type,
						}));
						subfields.push(object.name.clone() + "." + &field.name[..]);
					}
					types.push(json!({
						"id": object.name.clone(),
						"name": object.name.clone(),
						"kind": "OBJECT",
						"description": &object.description,
						"fields": subfields,
						"interfaces": []
					}));
				}
				TypeDefinition::Enum(object) => {
					let mut subvalues = Vec::new();
					for value in &object.values {
						enums.push(json!({
							"id": object.name.clone()+"."+ &value.name[..],
							"name": value.name,
							"description": value.description,
						}));
						subvalues.push(object.name.clone() + "." + &value.name[..]);
					}
					types.push(json!({
						"id": object.name.clone(),
						"name": object.name.clone(),
						"kind": "ENUM",
						"description": &object.description,
						"enumValues": subvalues,
						"interfaces": []
					}));
				}
				_ => {}
			},
			_ => {}
		}
	}
	let declared_types = types
		.iter()
		.map(|x| x["id"].clone())
		.collect::<Vec<serde_json::Value>>();

	for primitiv in vec!["ID", "Float", "Int", "String"] {
		types.push(json!({
			"id": primitiv.clone(),
			"name": primitiv.clone(),
			"kind": "SCALAR",
			"description": null,
			"interfaces": []
		}));
	}
	InstropectionParser {
		schema: traverse_schema("instropection.gql"),
		database: [
			(
				"__Schema".to_owned(),
				vec![json!({
					"id": "__Schema",
					"queryType": "Query",
					"types": declared_types,
					"directives": [],
					"mutationType": null,
				})],
			),
			("__Type".to_owned(), types),
			("__Field".to_owned(), fields),
			("__EnumValue".to_owned(), enums),
		]
		.iter()
		.cloned()
		.collect(),
	}
}
