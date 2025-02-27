use std::path::Path;

use clang::{Entity, EntityKind};

use crate::entity::WalkAction;

#[allow(unused)]
pub trait EntityWalkerVisitor<'tu> {
	fn wants_file(&mut self, path: &Path) -> bool {
		true
	}
	fn visit_entity(&mut self, entity: Entity<'tu>) -> WalkAction;
}

pub struct EntityWalker<'tu> {
	root_entity: Entity<'tu>,
}

impl<'tu> EntityWalker<'tu> {
	pub fn new(root_entity: Entity<'tu>) -> Self {
		Self { root_entity }
	}

	fn visit_cv_namespace(ns: Entity<'tu>, visitor: &mut impl EntityWalkerVisitor<'tu>) -> WalkAction {
		WalkAction::continue_until(ns.visit_children(|decl, _| {
			let res = match decl.get_kind() {
				EntityKind::Namespace => Self::visit_cv_namespace(decl, visitor),
				EntityKind::ClassDecl
				| EntityKind::ClassTemplate
				| EntityKind::ClassTemplatePartialSpecialization
				| EntityKind::StructDecl
				| EntityKind::EnumDecl
				| EntityKind::FunctionDecl
				| EntityKind::TypedefDecl
				| EntityKind::VarDecl
				| EntityKind::TypeAliasDecl => visitor.visit_entity(decl),
				EntityKind::Constructor
				| EntityKind::ConversionFunction
				| EntityKind::Destructor
				| EntityKind::Method
				| EntityKind::UnexposedDecl
				| EntityKind::FunctionTemplate
				| EntityKind::UsingDeclaration
				| EntityKind::UsingDirective
				| EntityKind::TypeAliasTemplateDecl => {
					/* ignoring */
					WalkAction::Continue
				}
				_ => {
					unreachable!("Unsupported decl for OpenCV namespace: {:#?}", decl)
				}
			};
			res.into()
		}))
	}

	pub fn walk_opencv_entities(&self, mut visitor: impl EntityWalkerVisitor<'tu>) {
		self.root_entity.visit_children(|root_decl, _| {
			let res = if let Some(loc) = root_decl.get_location() {
				if let Some(file) = loc.get_file_location().file.map(|f| f.get_path()) {
					if visitor.wants_file(&file) {
						match root_decl.get_kind() {
							EntityKind::Namespace => {
								if root_decl.get_name().map_or(false, |name| name.starts_with("cv")) {
									Self::visit_cv_namespace(root_decl, &mut visitor)
								} else {
									WalkAction::Continue
								}
							}
							EntityKind::MacroDefinition
							| EntityKind::MacroExpansion
							| EntityKind::EnumDecl
							| EntityKind::TypedefDecl => visitor.visit_entity(root_decl),
							EntityKind::FunctionDecl
							| EntityKind::InclusionDirective
							| EntityKind::UnionDecl
							| EntityKind::UnexposedDecl
							| EntityKind::StructDecl
							| EntityKind::Constructor
							| EntityKind::Method
							| EntityKind::FunctionTemplate
							| EntityKind::ConversionFunction
							| EntityKind::ClassTemplate
							| EntityKind::ClassDecl
							| EntityKind::Destructor
							| EntityKind::VarDecl => WalkAction::Continue,
							_ => {
								unreachable!("Unsupported decl for file: {:#?}", root_decl)
							}
						}
					} else {
						WalkAction::Continue
					}
				} else {
					WalkAction::Continue
				}
			} else {
				WalkAction::Continue
			};
			res.into()
		});
	}
}
