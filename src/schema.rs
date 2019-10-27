use graphql_parser::parse_schema;
use graphql_parser::schema::*;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
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

fn schema(file: &str) -> Document {
	let uri = String::from("public/") + file;
	let mut file = File::open(uri).expect("Unable to open");
	let mut data = String::new();
	file.read_to_string(&mut data).expect("Empty");
	parse_schema(&data).unwrap()
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
	let doc = schema(file);
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
	pub database: serde_json::Value,
}

pub fn build_schema_instropection() -> InstropectionParser {
	let mut fields = Vec::new();
	let mut types = Vec::new();
	let mut enums = Vec::new();
	let doc = schema("schema.gql");

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
							"id": object.name.clone()+"."+ &field.name[..],
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
		database: json!({
			"__Schema": [{
				"id": "__Schema",
				"queryType": "Query",
				"types": declared_types,
				"directives": [],
				"mutationType": null,
			}],
			"__Type": types,
			"__Field": fields,
			"__EnumValue": enums,
		}),
	}
}
