use serde_json::Value as JSONValue;
use std::collections::HashMap;

#[derive(serde_derive::Deserialize, serde_derive::Serialize, Clone, Debug)]
pub struct StructureIndex {
	pub name: String,
	pub objects: Vec<StructureType>,
	pub enums: Vec<StructureEnum>,
	#[serde(skip)]
	pub hashed_objects: HashMap<String, (usize, usize)>,
}

pub enum StructureItem<'a> {
	Object(&'a StructureType),
	Enum(&'a StructureEnum),
	None,
}
pub enum StructureItemMut<'a> {
	Object(&'a mut StructureType),
	Enum(&'a mut StructureEnum),
	None,
}

impl StructureIndex {
	pub fn perform_indexing(&mut self) {
		for (i, obj) in self.objects.iter_mut().enumerate() {
			for (j, fld) in obj.fields.iter_mut().enumerate() {
				obj.hashed_fields.insert(fld.name.clone(), j);
			}
			self.hashed_objects.insert(obj.name.clone(), (0, i));
		}
		for (i, obj) in self.enums.iter_mut().enumerate() {
			self.hashed_objects.insert(obj.name.clone(), (1, i));
		}
	}

	pub fn into_perform_indexing(mut self) -> StructureIndex {
		self.perform_indexing();
		self
	}

	pub fn find_object(&self, name: &str) -> StructureItem {
		match self.hashed_objects.get(name) {
			Some((0, v)) => StructureItem::Object(&self.objects[*v]),
			Some((1, v)) => StructureItem::Enum(&self.enums[*v]),
			_ => StructureItem::None,
		}
	}
	pub fn find_object_mut(&mut self, name: &str) -> StructureItemMut {
		match self.hashed_objects.get_mut(name) {
			Some((0, v)) => StructureItemMut::Object(self.objects.get_mut(*v).unwrap()),
			Some((1, v)) => StructureItemMut::Enum(self.enums.get_mut(*v).unwrap()),
			_ => StructureItemMut::None,
		}
	}
	pub fn add_object(&mut self, object: StructureType) {
		self.hashed_objects.insert(object.name.clone(), (0, self.objects.len()));
		self.objects.push(object);
	}
	pub fn add_enum(&mut self, object: StructureEnum) {
		self.hashed_objects.insert(object.name.clone(), (1, self.enums.len()));
		self.enums.push(object);
	}
}
#[derive(serde_derive::Deserialize, serde_derive::Serialize, Clone, Debug)]
pub struct StructureEnum {
	pub name: String,
	pub description: String,
	pub values: HashMap<String, JSONValue>,
}
#[derive(serde_derive::Deserialize, serde_derive::Serialize, Clone, Debug)]
pub struct StructureType {
	pub name: String,
	pub description: String,
	pub fields: Vec<StructureField>,
	#[serde(skip)]
	pub hashed_fields: HashMap<String, usize>,
}
impl StructureType {
	pub fn add_field(&mut self, field: StructureField) {
		self.hashed_fields.insert(field.name.clone(), self.fields.len());
		self.fields.push(field);
	}
	pub fn find_field(&self, name: &str) -> Option<&StructureField> {
		match self.hashed_fields.get(name) {
			Some(v) => Some(&self.fields[*v]),
			_ => None,
		}
	}
	pub fn find_field_mut(&mut self, name: &str) -> Option<&mut StructureField> {
		match self.hashed_fields.get(name) {
			Some(v) => Some(self.fields.get_mut(*v).unwrap()),
			_ => None,
		}
	}
}
#[derive(serde_derive::Deserialize, serde_derive::Serialize, Clone, Debug)]
pub struct StructureField {
	pub name: String,
	pub description: String,
	pub data_type: StructureDataType,
	pub return_type: StructureReturnType,
}

impl StructureField {
	pub fn from(name: String, description: String, kind: String, array: bool, resolver: Option<StructureDataResolver>) -> StructureField {
		StructureField {
			name: name,
			description: description,
			data_type: StructureDataType {
				resolver: resolver,
				kind: match &kind[..] {
					"ID" => "string",
					"Int" => "i32",
					"Float" => "f64",
					"String" => "string",
					"Boolean" => "bool",
					n @ _ => n,
				}.to_string(),
			},
			return_type: StructureReturnType {
				name: kind,
				is_array: array,
				is_nullable: false,
			},
		}
	}
}

#[derive(serde_derive::Deserialize, serde_derive::Serialize, Clone, Debug)]
pub struct StructureDataType {
	pub kind: String,
	pub resolver: Option<StructureDataResolver>,
}

#[derive(serde_derive::Deserialize, serde_derive::Serialize, Clone, Debug)]
pub struct StructureReturnType {
	pub name: String,
	pub is_array: bool,
	pub is_nullable: bool,
}

#[derive(serde_derive::Deserialize, serde_derive::Serialize, Clone, Debug)]
pub struct StructureDataDefault {
	pub kind: String,
	pub reference: JSONValue,
}

#[derive(serde_derive::Deserialize, serde_derive::Serialize, Clone, Debug)]
pub struct StructureDataResolver {
	pub kind: String,
	pub flags: Vec<String>,
	pub args: Vec<StructureReturnType>,
}
