use syn::{GenericArgument, PathArguments, Type};

pub(crate) fn option_inner_type(ty: &Type) -> Option<&Type> {
	let Type::Path(type_path) = ty else {
		return None;
	};
	if type_path.qself.is_some() {
		return None;
	}
	let segments = type_path.path.segments.iter().collect::<Vec<_>>();
	let supported = match segments.as_slice() {
		[option] => option.ident == "Option",
		[root, module, option] => {
			(root.ident == "std" || root.ident == "core")
				&& module.ident == "option"
				&& option.ident == "Option"
		}
		_ => false,
	};
	if !supported {
		return None;
	}
	let PathArguments::AngleBracketed(arguments) = &segments.last()?.arguments else {
		return None;
	};
	if arguments.args.len() != 1 {
		return None;
	}
	match arguments.args.first()? {
		GenericArgument::Type(inner) => Some(inner),
		_ => None,
	}
}
