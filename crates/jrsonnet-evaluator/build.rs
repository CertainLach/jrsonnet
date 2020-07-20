use bincode::serialize;
use jrsonnet_parser::{
	parse, Expr, FieldMember, FieldName, LocExpr, Member, ObjBody, ParserSettings,
};
use jrsonnet_stdlib::STDLIB_STR;
use std::{
	env,
	fs::File,
	io::Write,
	path::{Path, PathBuf},
	rc::Rc,
};
use structdump::CodegenResult;

fn main() {
	let parsed = parse(
		STDLIB_STR,
		&ParserSettings {
			file_name: Rc::new(PathBuf::from("std.jsonnet")),
			loc_data: true,
		},
	)
	.expect("parse");

	let parsed = if cfg!(feature = "faster") {
		let LocExpr(expr, location) = parsed;
		LocExpr(
			Rc::new(match Rc::try_unwrap(expr).unwrap() {
				Expr::Obj(ObjBody::MemberList(members)) => Expr::Obj(ObjBody::MemberList(
					members
						.into_iter()
						.filter(|p| {
							!matches!(
								p,
								Member::Field(FieldMember {
									name: FieldName::Fixed(name),
									..
								})
								if **name == *"join" || **name == *"manifestJsonEx" ||
								**name == *"escapeStringJson" || **name == *"equals" ||
								**name == *"base64" || **name == *"foldl" || **name == *"foldr" ||
								**name == *"sortImpl" || **name == *"range"
							)
						})
						.collect(),
				)),
				_ => panic!("std value should be object"),
			}),
			location,
		)
	} else {
		parsed
	};
	{
		let mut codegen = CodegenResult::default();
		let code = codegen.codegen(&parsed);

		let out_dir = env::var("OUT_DIR").unwrap();
		let dest_path = Path::new(&out_dir).join("stdlib.rs");
		let mut f = File::create(&dest_path).unwrap();
		f.write_all(&code.as_bytes()).unwrap();
	}
	{
		let out_dir = env::var("OUT_DIR").unwrap();
		let dest_path = Path::new(&out_dir).join("stdlib.bincode");
		let mut f = File::create(&dest_path).unwrap();
		f.write_all(&serialize(&parsed).unwrap()).unwrap();
	}
}
