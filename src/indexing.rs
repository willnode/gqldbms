use super::schema;
use super::parsing;
use serde_json::Value;
use std::collections::HashMap;
use std::convert::TryInto;

#[derive(Clone)]
pub enum FieldHashmaps {
	String(HashMap<String, Vec<usize>>),
	I32(HashMap<i32, Vec<usize>>),
	U64(HashMap<u64, Vec<usize>>),
	Null,
}

// Type, PropertyName (usually "id"), ValueOfProperty. returns index in database
// Valid for database lifetimes
pub type DatabaseHashmaps = HashMap<String, Option<HashMap<String, FieldHashmaps>>>;


pub fn subindex_keys<T, F>(classes: &Vec<Value>, converter: F) -> Vec<T>
where
	F: Fn(&Value) -> T,
	T: std::hash::Hash + Eq,
{
	let mut h = Vec::new();
	for value in classes {
		h.push(converter(&value));
	}
	h
}

pub fn subindex_hashmaps<T, F>(classes: &Vec<Value>, converter: F) -> HashMap<T, Vec<usize>>
where
	F: Fn(&Value) -> T,
	T: std::hash::Hash + Eq,
{
	let mut h : HashMap<T, Vec<usize>> = HashMap::new();
	for (index, value) in classes.iter().enumerate() {
		let key = converter(&value);
		match h.get_mut(&key) {
			Some(n) => { n.push(index); },
			None => { h.insert(key, vec![index]); }
		}
	}
	h
}

pub fn build_hashmaps(db: &parsing::DatabaseIndex, schema: &schema::SchemaClasses) -> DatabaseHashmaps {
	let mut hashes: DatabaseHashmaps = HashMap::new();
	for (name, classes) in db {
		if !schema.contains_key(name) {
			// No indexing needed, 'cause we can't infer exact type this way
			// Anyway if things doesn't exist in schema, it never be looked up, so don't panic.
			hashes.insert(name.clone(), Option::None);
		} else {
			// This type exist in schema. Let's index
			// WIP: Indexable props should be marked, but now let's just assume it's only ID for now.
			let arr_classes = classes;
			let (field_name, hash) = (
				"id".to_owned(),
				match &schema[name] {
					schema::SchemaType::Object(obj) => match obj.get("id") {
						Option::Some(field) => {
							if !field.data_type.is_indexed {
								panic!("id must be indexable!")
							} else {
								match &field.data_type.name_type[..] {
									"string" => FieldHashmaps::String(subindex_hashmaps(
										arr_classes,
										|value| value["id"].as_str().unwrap().to_string(),
									)),
									"i32" => FieldHashmaps::I32(subindex_hashmaps(
										arr_classes,
										|value| value["id"].as_i64().unwrap().try_into().unwrap(),
									)),
									"u64" => FieldHashmaps::U64(subindex_hashmaps(
										arr_classes,
										|value| value["id"].as_u64().unwrap(),
									)),
									_ => panic!("id is not indexable!"),
								}
							}
						}
						_ => panic!("id is not exist in one of schema class!"),
					},
					_ => panic!(
						"An object is exist in DB, but in schema it's refered as something else"
					),
				},
			);

			let mut type_hash = HashMap::new();
			type_hash.insert(field_name, hash);
			hashes.insert(name.clone(), Some(type_hash));
		}
	}
	hashes
}
