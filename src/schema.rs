use graphql_parser::parse_schema;
use graphql_parser::schema::*;
use std::fs::File;
use std::io::Read;
use std::collections::HashMap;
use std::ops::Deref;

#[derive(Debug, Clone, PartialEq)]
pub struct FieldType {
	pub is_array: bool,
	pub is_array_non_nullable: bool,
	pub name_type: String,
	pub is_type_non_nullable: bool,
}

pub fn get_field_type(t: &Type) -> FieldType {
	let mut r = FieldType {
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
			},
			Type::NonNullType(tt) => {
				r.is_type_non_nullable = true;
				tt.deref().clone()
			},
			Type::NamedType(name) => {
				r.name_type.push_str(name);
				break;
			},
		};
	}
	r
}

pub type SchemaClasses = HashMap<String, SchemaType>;
pub type SchemaFields = HashMap<String, FieldType>;

fn schema() -> Document
{
	let uri = String::from("public/schema.gql");
    let mut file = File::open(uri).expect("Unable to open");
    let mut data = String::new();
    file.read_to_string(&mut data).expect("Empty");
	parse_schema(&data).unwrap()
}

fn traverse_object(object: &ObjectType) -> SchemaFields
{
	let mut fields = HashMap::new();
	for field in &object.fields {
		fields.insert(field.name.clone(), get_field_type(&field.field_type));
	}
	fields
}

pub enum SchemaType {
	Object(SchemaFields)
}


pub fn traverse_schema() -> SchemaClasses
{
	let doc = schema();
	let mut hashes = HashMap::new();
	for def in doc.definitions {
		match &def {
			Definition::TypeDefinition(typedef) => {
				match &typedef {
					TypeDefinition::Scalar(_) => {
					}
					TypeDefinition::Object(object) => {
						hashes.insert(object.name.clone(), SchemaType::Object(traverse_object(&object)));
					}
					_ => {

					},
				}
			},
			_ => {},
		}
	}
	hashes
}