use super::{parsing, structure};
use graphql_parser::schema::{Definition, Document, ObjectType, Type, TypeDefinition};
use serde_json::Value as JSONValue;
use std::collections::HashMap;
use std::ops::Deref;

pub fn get_field_type(t: &Type) -> structure::StructureReturnType {
	let mut r = structure::StructureReturnType {
		is_array: false,
		name: String::new(),
		is_nullable: true,
	};
	let mut t = t.clone();
	loop {
		t = match &t {
			Type::ListType(tt) => {
				r.is_array = true;
				tt.deref().clone()
			}
			Type::NonNullType(tt) => {
				r.is_nullable = false;
				tt.deref().clone()
			}
			Type::NamedType(name) => {
				r.name.push_str(name);
				break;
			}
		};
	}
	r
}

pub fn get_data_type(
	t: &structure::StructureReturnType,
	d: &str,
	n: &str,
) -> structure::StructureDataType {
	let mut data_type = match t.name.as_ref() {
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
	structure::StructureDataType {
		resolver: match n {
			"Query" => Some(structure::StructureDataResolver {
				kind: "ALL_REFERENCES".to_owned(),
				args: Vec::default(),
				flags: Vec::default(),
			}),
			"Mutation" => match &n[0..6] {
				"create" => Some(structure::StructureDataResolver {
					kind: "CREATE".to_owned(),
					args: Vec::default(),
					flags: vec![n[6..].to_owned()],
				}),
				"update" => Some(structure::StructureDataResolver {
					kind: "UPDATE".to_owned(),
					args: Vec::default(),
					flags: vec![n[6..].to_owned()],
				}),
				"delete" => Some(structure::StructureDataResolver {
					kind: "DELETE".to_owned(),
					args: Vec::default(),
					flags: vec![n[6..].to_owned()],
				}),
				_ => None,
			},
			_ => None,
		},
		kind: data_type.to_owned(),
	}
}

fn traverse_object(object: &ObjectType) -> structure::StructureType {
	let mut fields = Vec::new();
	for field in &object.fields {
		let return_type = get_field_type(&field.field_type);
		let description = match &field.description {
			Some(v) => v.clone(),
			_ => String::new(),
		};
		let data_type = get_data_type(&return_type, description.as_ref(), object.name.as_ref());
		fields.push(structure::StructureField {
			name: field.name.clone(),
			description: description,
			return_type: return_type,
			data_type: data_type,
		});
	}
	structure::StructureType {
		name: object.name.clone(),
		description: object
			.description
			.as_ref()
			.unwrap_or(&"".to_owned())
			.clone(),
		fields: fields,
		hashed_fields: HashMap::new(),
	}
}

pub fn traverse_schema(doc: &Document) -> structure::StructureIndex {
	let mut objects = Vec::new();
	let mut enums = Vec::new();
	let mut scalars = Vec::new();
	for def in &doc.definitions {
		match &def {
			Definition::TypeDefinition(typedef) => match &typedef {
				TypeDefinition::Scalar(scl) => scalars.push(structure::StructureScalar {
					name: scl.name.clone(),
					description: scl.description.as_ref().unwrap_or(&"".to_owned()).clone(),
				}),
				TypeDefinition::Object(object) => {
					objects.push(traverse_object(&object));
				}
				TypeDefinition::Enum(enu) => {
					let enus = enu
						.values
						.iter()
						.map(|x| (x.name.clone(), json!(x.name)))
						.collect::<HashMap<String, JSONValue>>();
					enums.push(structure::StructureEnum {
						name: enu.name.clone(),
						description: enu.description.as_ref().unwrap_or(&"".to_owned()).clone(),
						values: enus,
					});
				}
				_ => {}
			},
			_ => {}
		}
	}
	(structure::StructureIndex {
		name: "".to_owned(),
		objects: objects,
		enums: enums,
		scalars: scalars,
		hashed_objects: HashMap::new(),
	})
	.into_perform_indexing()
}

pub struct InstropectionParser {
	pub schema: structure::StructureIndex,
	pub database: parsing::DatabaseIndex,
}

pub fn build_schema_instropection(
	doc: &structure::StructureIndex,
	instropection: structure::StructureIndex,
) -> InstropectionParser {
	let mut fields = Vec::new();
	let mut types = Vec::new();
	let mut enums = Vec::new();

	for object in &doc.scalars {
		types.push(json!({
			"id": object.name.clone(),
			"name": object.name.clone(),
			"kind": "SCALAR",
			"description": &object.description,
			"interfaces": []
		}));
	}

	for object in &doc.objects {
		let mut subfields = Vec::new();
		for field in &object.fields {
			fields.push(json!({
				"id": format!("{}.{}", object.name, field.name),
				"name": field.name,
				"description": field.description,
				"isDeprecated": false,
				"args": [],
				"type": field.return_type.name.clone(),
			}));
			subfields.push(object.name.clone() + "." + field.name.as_ref());
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

	for object in &doc.enums {
		let mut subvalues = Vec::new();
		for (key, _) in &object.values {
			enums.push(json!({
				"id": object.name.clone()+"."+ key.as_ref(),
				"name": key.clone(),
				"description": "".to_owned(),
			}));
			subvalues.push(object.name.clone() + "." + key.as_ref());
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

	let declared_types = types
		.iter()
		.map(|x| x["id"].clone())
		.collect::<Vec<serde_json::Value>>();

	for primitiv in vec!["ID", "Float", "Int", "String", "Boolean"] {
		types.push(json!({
			"id": primitiv.clone(),
			"name": primitiv.clone(),
			"kind": "SCALAR",
			"description": null,
			"interfaces": []
		}));
	}
	InstropectionParser {
		schema: instropection,
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
