use std::borrow::Cow;
use std::rc::Rc;

use crate::debug::DefinitionLocation;
use crate::field::Field;
use crate::func::ReturnKind;
use crate::type_ref::{Constness, TypeRef, TypeRefDesc};
use crate::{Class, Func, FuncTypeHint};

use super::FuncKind;

#[derive(Clone, Debug)]
pub struct FuncDesc<'tu, 'ge> {
	pub kind: FuncKind<'tu, 'ge>,
	pub type_hint: FuncTypeHint,
	pub constness: Constness,
	// fixme, this should be just a `is_infallible` property, but `method_get` in `Vector` forces `InfallibleViaArg`
	pub return_kind: ReturnKind,
	pub cpp_fullname: Rc<str>,
	pub custom_rust_leafname: Option<Rc<str>>,
	pub rust_module: Rc<str>,
	pub doc_comment: Rc<str>,
	pub def_loc: DefinitionLocation,
	pub arguments: Rc<[Field<'tu, 'ge>]>,
	pub return_type_ref: TypeRef<'tu, 'ge>,
	pub cpp_body: FuncCppBody,
}

impl<'tu, 'ge> FuncDesc<'tu, 'ge> {
	pub fn new(
		kind: FuncKind<'tu, 'ge>,
		constness: Constness,
		return_kind: ReturnKind,
		cpp_fullname: impl Into<Rc<str>>,
		rust_module: impl Into<Rc<str>>,
		arguments: impl Into<Rc<[Field<'tu, 'ge>]>>,
		cpp_body: FuncCppBody,
		return_type_ref: TypeRef<'tu, 'ge>,
	) -> Self {
		#![allow(clippy::too_many_arguments)]
		Self {
			kind,
			type_hint: FuncTypeHint::None,
			constness,
			return_kind,
			cpp_fullname: cpp_fullname.into(),
			custom_rust_leafname: None,
			rust_module: rust_module.into(),
			doc_comment: "".into(),
			def_loc: DefinitionLocation::Generated,
			arguments: arguments.into(),
			return_type_ref,
			cpp_body,
		}
	}

	pub fn method_delete(rust_local: &str, class_desc: Class<'tu, 'ge>) -> Func<'tu, 'ge> {
		Func::new_desc(FuncDesc::new(
			FuncKind::InstanceMethod(class_desc),
			Constness::Mut,
			ReturnKind::InfallibleNaked,
			format!("cv::{rust_local}::delete"),
			"<unused>",
			vec![],
			FuncCppBody::ManualCall("delete instance".into()),
			TypeRefDesc::void(),
		))
	}
}

#[derive(Clone, Debug)]
pub enum FuncCppBody {
	/// Handle the call automatically based on the function context, usually just forwards to the corresponding OpenCV function
	Auto,
	/// Specify manual call, use the automatic return handling (e.g. `Mat ret = <manual_call>`)
	ManualCall(Cow<'static, str>),
	/// Specify full manual function body including the return
	ManualFull(Cow<'static, str>),
}
