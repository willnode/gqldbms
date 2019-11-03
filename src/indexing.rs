use super::parsing;
use super::structure;
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
	let mut h: HashMap<T, Vec<usize>> = HashMap::new();
	for (index, value) in classes.iter().enumerate() {
		let key = converter(&value);
		match h.get_mut(&key) {
			Some(n) => {
				n.push(index);
			}
			None => {
				h.insert(key, vec![index]);
			}
		}
	}
	h
}

pub fn build_hashmaps(
	db: &parsing::DatabaseIndex,
	schema: &structure::StructureIndex,
) -> DatabaseHashmaps {
	let mut hashes: DatabaseHashmaps = HashMap::new();
	for obj in &schema.objects {
		// This type exist in schema. Let's index
		// WIP: Indexable props should be marked, but now let's just assume it's only ID for now.
		let arr_classes = match db.get(&obj.name) {
			Some(v) => v,
			_ => {
				hashes.insert(obj.name.clone(), Option::None);
				continue;
			}
		};
		let (field_name, hash) = (
			"id".to_owned(),
			match obj.find_field("id") {
				Option::Some(field) => match field.data_type.kind.as_ref() {
					"string" => FieldHashmaps::String(subindex_hashmaps(arr_classes, |value| {
						value["id"].as_str().unwrap().to_string()
					})),
					"i32" => FieldHashmaps::I32(subindex_hashmaps(arr_classes, |value| {
						value["id"].as_i64().unwrap().try_into().unwrap()
					})),
					"u64" => FieldHashmaps::U64(subindex_hashmaps(arr_classes, |value| {
						value["id"].as_u64().unwrap()
					})),
					_ => panic!("id is not indexable!"),
				},
				_ => {
					hashes.insert(obj.name.clone(), Option::None);
					continue;
				}
			},
		);

		let mut type_hash = HashMap::new();
		type_hash.insert(field_name, hash);
		hashes.insert(obj.name.clone(), Some(type_hash));
	}
	hashes
}
