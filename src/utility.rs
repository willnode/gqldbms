use std::fs::File;
use std::io::Read;

pub fn read_pub_file(path: &str) -> String {
	let uri = String::from("public/") + path;
	let mut file = File::open(uri).expect(&format!("Unable to open `public/{}`", path)[..]);
	let mut data = String::new();
	file.read_to_string(&mut data).expect(&format!("Unable to read `public/{}` (Invalid UTF-8 file?)", path)[..]);
	data
}