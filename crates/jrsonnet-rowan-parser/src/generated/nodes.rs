//! This is a generated file, please do not edit manually. Changes can be
//! made in codegeneration that lives in `xtask` top-level dir.

#![allow(non_snake_case, clippy::match_like_matches_macro)]
use crate::{
	ast::{support, AstChildren, AstNode, AstToken},
	SyntaxKind::{self, *},
	SyntaxNode, SyntaxToken, T,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceFile {
	pub(crate) syntax: SyntaxNode,
}
impl SourceFile {
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprBinary {
	pub(crate) syntax: SyntaxNode,
}
impl ExprBinary {
	pub fn lhs(&self) -> Option<LhsExpr> {
		support::child(&self.syntax)
	}
	pub fn binary_operator(&self) -> Option<BinaryOperator> {
		support::token_child(&self.syntax)
	}
	pub fn rhs(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LhsExpr {
	pub(crate) syntax: SyntaxNode,
}
impl LhsExpr {
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprUnary {
	pub(crate) syntax: SyntaxNode,
}
impl ExprUnary {
	pub fn unary_operator(&self) -> Option<UnaryOperator> {
		support::token_child(&self.syntax)
	}
	pub fn rhs(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprSlice {
	pub(crate) syntax: SyntaxNode,
}
impl ExprSlice {
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
	pub fn slice_desc(&self) -> Option<SliceDesc> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SliceDesc {
	pub(crate) syntax: SyntaxNode,
}
impl SliceDesc {
	pub fn l_brack_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!['['])
	}
	pub fn from(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
	pub fn colon_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![:])
	}
	pub fn end(&self) -> Option<SliceDescEnd> {
		support::child(&self.syntax)
	}
	pub fn step(&self) -> Option<SliceDescStep> {
		support::child(&self.syntax)
	}
	pub fn r_brack_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![']'])
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprIndex {
	pub(crate) syntax: SyntaxNode,
}
impl ExprIndex {
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
	pub fn dot_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![.])
	}
	pub fn index(&self) -> Option<Name> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Name {
	pub(crate) syntax: SyntaxNode,
}
impl Name {
	pub fn ident_lit(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, IDENT)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprIndexExpr {
	pub(crate) syntax: SyntaxNode,
}
impl ExprIndexExpr {
	pub fn base(&self) -> Option<LhsExpr> {
		support::child(&self.syntax)
	}
	pub fn l_brack_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!['['])
	}
	pub fn index(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
	pub fn r_brack_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![']'])
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprApply {
	pub(crate) syntax: SyntaxNode,
}
impl ExprApply {
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
	pub fn args_desc(&self) -> Option<ArgsDesc> {
		support::child(&self.syntax)
	}
	pub fn tailstrict_kw_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![tailstrict])
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArgsDesc {
	pub(crate) syntax: SyntaxNode,
}
impl ArgsDesc {
	pub fn l_paren_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!['('])
	}
	pub fn args(&self) -> AstChildren<Arg> {
		support::children(&self.syntax)
	}
	pub fn r_paren_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![')'])
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprObjExtend {
	pub(crate) syntax: SyntaxNode,
}
impl ExprObjExtend {
	pub fn lhs_expr(&self) -> Option<LhsExpr> {
		support::child(&self.syntax)
	}
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprParened {
	pub(crate) syntax: SyntaxNode,
}
impl ExprParened {
	pub fn l_paren_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!['('])
	}
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
	pub fn r_paren_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![')'])
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprLiteral {
	pub(crate) syntax: SyntaxNode,
}
impl ExprLiteral {
	pub fn literal(&self) -> Option<Literal> {
		support::token_child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprIntrinsicThisFile {
	pub(crate) syntax: SyntaxNode,
}
impl ExprIntrinsicThisFile {
	pub fn intrinsic_this_file_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!["$intrinsicThisFile"])
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprIntrinsicId {
	pub(crate) syntax: SyntaxNode,
}
impl ExprIntrinsicId {
	pub fn intrinsic_id_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!["$intrinsicId"])
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprIntrinsic {
	pub(crate) syntax: SyntaxNode,
}
impl ExprIntrinsic {
	pub fn intrinsic_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!["$intrinsic"])
	}
	pub fn l_paren_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!['('])
	}
	pub fn name(&self) -> Option<Name> {
		support::child(&self.syntax)
	}
	pub fn r_paren_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![')'])
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprString {
	pub(crate) syntax: SyntaxNode,
}
impl ExprString {
	pub fn text(&self) -> Option<Text> {
		support::token_child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprNumber {
	pub(crate) syntax: SyntaxNode,
}
impl ExprNumber {
	pub fn number(&self) -> Option<Number> {
		support::token_child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprArray {
	pub(crate) syntax: SyntaxNode,
}
impl ExprArray {
	pub fn l_brack_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!['['])
	}
	pub fn exprs(&self) -> AstChildren<Expr> {
		support::children(&self.syntax)
	}
	pub fn r_brack_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![']'])
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprObject {
	pub(crate) syntax: SyntaxNode,
}
impl ExprObject {
	pub fn l_brace_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!['{'])
	}
	pub fn obj_body(&self) -> Option<ObjBody> {
		support::child(&self.syntax)
	}
	pub fn r_brace_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!['}'])
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprArrayComp {
	pub(crate) syntax: SyntaxNode,
}
impl ExprArrayComp {
	pub fn l_brack_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!['['])
	}
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
	pub fn comma_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![,])
	}
	pub fn comp_specs(&self) -> AstChildren<CompSpec> {
		support::children(&self.syntax)
	}
	pub fn r_brack_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![']'])
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprImport {
	pub(crate) syntax: SyntaxNode,
}
impl ExprImport {
	pub fn import_kind(&self) -> Option<ImportKind> {
		support::token_child(&self.syntax)
	}
	pub fn text(&self) -> Option<Text> {
		support::token_child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprVar {
	pub(crate) syntax: SyntaxNode,
}
impl ExprVar {
	pub fn name(&self) -> Option<Name> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprLocal {
	pub(crate) syntax: SyntaxNode,
}
impl ExprLocal {
	pub fn local_kw_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![local])
	}
	pub fn binds(&self) -> AstChildren<Bind> {
		support::children(&self.syntax)
	}
	pub fn semi_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![;])
	}
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprIfThenElse {
	pub(crate) syntax: SyntaxNode,
}
impl ExprIfThenElse {
	pub fn if_kw_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![if])
	}
	pub fn cond(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
	pub fn then_kw_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![then])
	}
	pub fn then(&self) -> Option<TrueExpr> {
		support::child(&self.syntax)
	}
	pub fn else_kw_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![else])
	}
	pub fn else_(&self) -> Option<FalseExpr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrueExpr {
	pub(crate) syntax: SyntaxNode,
}
impl TrueExpr {
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FalseExpr {
	pub(crate) syntax: SyntaxNode,
}
impl FalseExpr {
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprFunction {
	pub(crate) syntax: SyntaxNode,
}
impl ExprFunction {
	pub fn function_kw_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![function])
	}
	pub fn l_paren_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!['('])
	}
	pub fn params_desc(&self) -> Option<ParamsDesc> {
		support::child(&self.syntax)
	}
	pub fn r_paren_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![')'])
	}
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParamsDesc {
	pub(crate) syntax: SyntaxNode,
}
impl ParamsDesc {
	pub fn l_paren_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!['('])
	}
	pub fn params(&self) -> AstChildren<Param> {
		support::children(&self.syntax)
	}
	pub fn r_paren_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![')'])
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprAssert {
	pub(crate) syntax: SyntaxNode,
}
impl ExprAssert {
	pub fn assertion(&self) -> Option<Assertion> {
		support::child(&self.syntax)
	}
	pub fn semi_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![;])
	}
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Assertion {
	pub(crate) syntax: SyntaxNode,
}
impl Assertion {
	pub fn assert_kw_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![assert])
	}
	pub fn condition(&self) -> Option<LhsExpr> {
		support::child(&self.syntax)
	}
	pub fn colon_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![:])
	}
	pub fn message(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExprError {
	pub(crate) syntax: SyntaxNode,
}
impl ExprError {
	pub fn error_kw_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![error])
	}
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SliceDescEnd {
	pub(crate) syntax: SyntaxNode,
}
impl SliceDescEnd {
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SliceDescStep {
	pub(crate) syntax: SyntaxNode,
}
impl SliceDescStep {
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Arg {
	pub(crate) syntax: SyntaxNode,
}
impl Arg {
	pub fn name(&self) -> Option<Name> {
		support::child(&self.syntax)
	}
	pub fn assign_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![=])
	}
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjBodyComp {
	pub(crate) syntax: SyntaxNode,
}
impl ObjBodyComp {
	pub fn pre(&self) -> AstChildren<ObjLocalPostComma> {
		support::children(&self.syntax)
	}
	pub fn l_brack_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!['['])
	}
	pub fn key(&self) -> Option<LhsExpr> {
		support::child(&self.syntax)
	}
	pub fn r_brack_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![']'])
	}
	pub fn plus_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![+])
	}
	pub fn colon_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![:])
	}
	pub fn value(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
	pub fn post(&self) -> AstChildren<ObjLocalPreComma> {
		support::children(&self.syntax)
	}
	pub fn comp_specs(&self) -> AstChildren<CompSpec> {
		support::children(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjLocalPostComma {
	pub(crate) syntax: SyntaxNode,
}
impl ObjLocalPostComma {
	pub fn obj_local(&self) -> Option<ObjLocal> {
		support::child(&self.syntax)
	}
	pub fn comma_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![,])
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjLocalPreComma {
	pub(crate) syntax: SyntaxNode,
}
impl ObjLocalPreComma {
	pub fn comma_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![,])
	}
	pub fn obj_local(&self) -> Option<ObjLocal> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjBodyMemberList {
	pub(crate) syntax: SyntaxNode,
}
impl ObjBodyMemberList {
	pub fn members(&self) -> AstChildren<Member> {
		support::children(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjLocal {
	pub(crate) syntax: SyntaxNode,
}
impl ObjLocal {
	pub fn local_kw_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![local])
	}
	pub fn bind(&self) -> Option<Bind> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemberBindStmt {
	pub(crate) syntax: SyntaxNode,
}
impl MemberBindStmt {
	pub fn obj_local(&self) -> Option<ObjLocal> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemberAssertStmt {
	pub(crate) syntax: SyntaxNode,
}
impl MemberAssertStmt {
	pub fn assertion(&self) -> Option<Assertion> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemberField {
	pub(crate) syntax: SyntaxNode,
}
impl MemberField {
	pub fn field(&self) -> Option<Field> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldNormal {
	pub(crate) syntax: SyntaxNode,
}
impl FieldNormal {
	pub fn field_name(&self) -> Option<FieldName> {
		support::child(&self.syntax)
	}
	pub fn plus_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![+])
	}
	pub fn visibility(&self) -> Option<Visibility> {
		support::token_child(&self.syntax)
	}
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldMethod {
	pub(crate) syntax: SyntaxNode,
}
impl FieldMethod {
	pub fn field_name(&self) -> Option<FieldName> {
		support::child(&self.syntax)
	}
	pub fn params_desc(&self) -> Option<ParamsDesc> {
		support::child(&self.syntax)
	}
	pub fn visibility(&self) -> Option<Visibility> {
		support::token_child(&self.syntax)
	}
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldNameFixed {
	pub(crate) syntax: SyntaxNode,
}
impl FieldNameFixed {
	pub fn id(&self) -> Option<Name> {
		support::child(&self.syntax)
	}
	pub fn text(&self) -> Option<Text> {
		support::token_child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldNameDynamic {
	pub(crate) syntax: SyntaxNode,
}
impl FieldNameDynamic {
	pub fn l_brack_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!['['])
	}
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
	pub fn r_brack_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![']'])
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForSpec {
	pub(crate) syntax: SyntaxNode,
}
impl ForSpec {
	pub fn for_kw_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![for])
	}
	pub fn bind(&self) -> Option<Name> {
		support::child(&self.syntax)
	}
	pub fn in_kw_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![in])
	}
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IfSpec {
	pub(crate) syntax: SyntaxNode,
}
impl IfSpec {
	pub fn if_kw_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![if])
	}
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BindDestruct {
	pub(crate) syntax: SyntaxNode,
}
impl BindDestruct {
	pub fn into(&self) -> Option<Destruct> {
		support::child(&self.syntax)
	}
	pub fn assign_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![=])
	}
	pub fn value(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BindFunction {
	pub(crate) syntax: SyntaxNode,
}
impl BindFunction {
	pub fn name(&self) -> Option<Name> {
		support::child(&self.syntax)
	}
	pub fn params(&self) -> Option<ParamsDesc> {
		support::child(&self.syntax)
	}
	pub fn assign_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![=])
	}
	pub fn value(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Param {
	pub(crate) syntax: SyntaxNode,
}
impl Param {
	pub fn destruct(&self) -> Option<Destruct> {
		support::child(&self.syntax)
	}
	pub fn assign_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![=])
	}
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DestructFull {
	pub(crate) syntax: SyntaxNode,
}
impl DestructFull {
	pub fn name(&self) -> Option<Name> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DestructSkip {
	pub(crate) syntax: SyntaxNode,
}
impl DestructSkip {
	pub fn question_mark_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![?])
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DestructArray {
	pub(crate) syntax: SyntaxNode,
}
impl DestructArray {
	pub fn l_brack_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!['['])
	}
	pub fn destruct_array_parts(&self) -> AstChildren<DestructArrayPart> {
		support::children(&self.syntax)
	}
	pub fn r_brack_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![']'])
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DestructObject {
	pub(crate) syntax: SyntaxNode,
}
impl DestructObject {
	pub fn l_brace_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!['{'])
	}
	pub fn destruct_object_fields(&self) -> AstChildren<DestructObjectField> {
		support::children(&self.syntax)
	}
	pub fn destruct_rest(&self) -> Option<DestructRest> {
		support::child(&self.syntax)
	}
	pub fn comma_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![,])
	}
	pub fn r_brace_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T!['}'])
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DestructObjectField {
	pub(crate) syntax: SyntaxNode,
}
impl DestructObjectField {
	pub fn field(&self) -> Option<Name> {
		support::child(&self.syntax)
	}
	pub fn colon_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![:])
	}
	pub fn destruct(&self) -> Option<Destruct> {
		support::child(&self.syntax)
	}
	pub fn assign_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![=])
	}
	pub fn expr(&self) -> Option<Expr> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DestructRest {
	pub(crate) syntax: SyntaxNode,
}
impl DestructRest {
	pub fn dotdotdot_token(&self) -> Option<SyntaxToken> {
		support::token(&self.syntax, T![...])
	}
	pub fn into(&self) -> Option<Name> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DestructArrayElement {
	pub(crate) syntax: SyntaxNode,
}
impl DestructArrayElement {
	pub fn destruct(&self) -> Option<Destruct> {
		support::child(&self.syntax)
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
	ExprBinary(ExprBinary),
	ExprUnary(ExprUnary),
	ExprSlice(ExprSlice),
	ExprIndex(ExprIndex),
	ExprIndexExpr(ExprIndexExpr),
	ExprApply(ExprApply),
	ExprObjExtend(ExprObjExtend),
	ExprParened(ExprParened),
	ExprIntrinsicThisFile(ExprIntrinsicThisFile),
	ExprIntrinsicId(ExprIntrinsicId),
	ExprIntrinsic(ExprIntrinsic),
	ExprString(ExprString),
	ExprNumber(ExprNumber),
	ExprLiteral(ExprLiteral),
	ExprArray(ExprArray),
	ExprObject(ExprObject),
	ExprArrayComp(ExprArrayComp),
	ExprImport(ExprImport),
	ExprVar(ExprVar),
	ExprLocal(ExprLocal),
	ExprIfThenElse(ExprIfThenElse),
	ExprFunction(ExprFunction),
	ExprAssert(ExprAssert),
	ExprError(ExprError),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ObjBody {
	ObjBodyComp(ObjBodyComp),
	ObjBodyMemberList(ObjBodyMemberList),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CompSpec {
	ForSpec(ForSpec),
	IfSpec(IfSpec),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Bind {
	BindDestruct(BindDestruct),
	BindFunction(BindFunction),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Member {
	MemberBindStmt(MemberBindStmt),
	MemberAssertStmt(MemberAssertStmt),
	MemberField(MemberField),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Field {
	FieldNormal(FieldNormal),
	FieldMethod(FieldMethod),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FieldName {
	FieldNameFixed(FieldNameFixed),
	FieldNameDynamic(FieldNameDynamic),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Destruct {
	DestructFull(DestructFull),
	DestructSkip(DestructSkip),
	DestructArray(DestructArray),
	DestructObject(DestructObject),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DestructArrayPart {
	DestructArrayElement(DestructArrayElement),
	DestructRest(DestructRest),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BinaryOperator {
	syntax: SyntaxToken,
	kind: BinaryOperatorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOperatorKind {
	Or,
	And,
	BitOr,
	BitXor,
	BitAnd,
	Eq,
	Ne,
	Lt,
	Gt,
	Le,
	Ge,
	InKw,
	Lhs,
	Rhs,
	Plus,
	Minus,
	Mul,
	Div,
	Modulo,
	ErrorNoOperator,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnaryOperator {
	syntax: SyntaxToken,
	kind: UnaryOperatorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOperatorKind {
	Minus,
	Not,
	BitNot,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Literal {
	syntax: SyntaxToken,
	kind: LiteralKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LiteralKind {
	NullKw,
	TrueKw,
	FalseKw,
	SelfKw,
	Dollar,
	SuperKw,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Text {
	syntax: SyntaxToken,
	kind: TextKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextKind {
	StringDouble,
	ErrorStringDoubleUnterminated,
	StringSingle,
	ErrorStringSingleUnterminated,
	StringDoubleVerbatim,
	ErrorStringDoubleVerbatimUnterminated,
	StringSingleVerbatim,
	ErrorStringSingleVerbatimUnterminated,
	ErrorStringVerbatimMissingQuotes,
	StringBlock,
	ErrorStringBlockUnexpectedEnd,
	ErrorStringBlockMissingNewLine,
	ErrorStringBlockMissingTermination,
	ErrorStringBlockMissingIndent,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Number {
	syntax: SyntaxToken,
	kind: NumberKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumberKind {
	Float,
	ErrorFloatJunkAfterPoint,
	ErrorFloatJunkAfterExponent,
	ErrorFloatJunkAfterExponentSign,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImportKind {
	syntax: SyntaxToken,
	kind: ImportKindKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportKindKind {
	ImportstrKw,
	ImportbinKw,
	ImportKw,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Visibility {
	syntax: SyntaxToken,
	kind: VisibilityKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VisibilityKind {
	Coloncoloncolon,
	Coloncolon,
	Colon,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Trivia {
	syntax: SyntaxToken,
	kind: TriviaKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TriviaKind {
	Whitespace,
	MultiLineComment,
	ErrorCommentTooShort,
	ErrorCommentUnterminated,
	SingleLineHashComment,
	SingleLineSlashComment,
}
impl AstNode for SourceFile {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == SOURCE_FILE
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprBinary {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_BINARY
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for LhsExpr {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == LHS_EXPR
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprUnary {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_UNARY
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprSlice {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_SLICE
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for SliceDesc {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == SLICE_DESC
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprIndex {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_INDEX
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for Name {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == NAME
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprIndexExpr {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_INDEX_EXPR
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprApply {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_APPLY
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ArgsDesc {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == ARGS_DESC
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprObjExtend {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_OBJ_EXTEND
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprParened {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_PARENED
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprLiteral {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_LITERAL
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprIntrinsicThisFile {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_INTRINSIC_THIS_FILE
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprIntrinsicId {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_INTRINSIC_ID
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprIntrinsic {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_INTRINSIC
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprString {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_STRING
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprNumber {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_NUMBER
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprArray {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_ARRAY
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprObject {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_OBJECT
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprArrayComp {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_ARRAY_COMP
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprImport {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_IMPORT
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprVar {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_VAR
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprLocal {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_LOCAL
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprIfThenElse {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_IF_THEN_ELSE
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for TrueExpr {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == TRUE_EXPR
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for FalseExpr {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == FALSE_EXPR
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprFunction {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_FUNCTION
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ParamsDesc {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == PARAMS_DESC
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprAssert {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_ASSERT
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for Assertion {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == ASSERTION
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ExprError {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == EXPR_ERROR
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for SliceDescEnd {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == SLICE_DESC_END
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for SliceDescStep {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == SLICE_DESC_STEP
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for Arg {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == ARG
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ObjBodyComp {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == OBJ_BODY_COMP
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ObjLocalPostComma {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == OBJ_LOCAL_POST_COMMA
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ObjLocalPreComma {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == OBJ_LOCAL_PRE_COMMA
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ObjBodyMemberList {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == OBJ_BODY_MEMBER_LIST
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ObjLocal {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == OBJ_LOCAL
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for MemberBindStmt {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == MEMBER_BIND_STMT
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for MemberAssertStmt {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == MEMBER_ASSERT_STMT
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for MemberField {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == MEMBER_FIELD
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for FieldNormal {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == FIELD_NORMAL
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for FieldMethod {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == FIELD_METHOD
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for FieldNameFixed {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == FIELD_NAME_FIXED
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for FieldNameDynamic {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == FIELD_NAME_DYNAMIC
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for ForSpec {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == FOR_SPEC
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for IfSpec {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == IF_SPEC
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for BindDestruct {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == BIND_DESTRUCT
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for BindFunction {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == BIND_FUNCTION
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for Param {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == PARAM
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for DestructFull {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == DESTRUCT_FULL
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for DestructSkip {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == DESTRUCT_SKIP
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for DestructArray {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == DESTRUCT_ARRAY
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for DestructObject {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == DESTRUCT_OBJECT
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for DestructObjectField {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == DESTRUCT_OBJECT_FIELD
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for DestructRest {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == DESTRUCT_REST
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl AstNode for DestructArrayElement {
	fn can_cast(kind: SyntaxKind) -> bool {
		kind == DESTRUCT_ARRAY_ELEMENT
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		if Self::can_cast(syntax.kind()) {
			Some(Self { syntax })
		} else {
			None
		}
	}
	fn syntax(&self) -> &SyntaxNode {
		&self.syntax
	}
}
impl From<ExprBinary> for Expr {
	fn from(node: ExprBinary) -> Expr {
		Expr::ExprBinary(node)
	}
}
impl From<ExprUnary> for Expr {
	fn from(node: ExprUnary) -> Expr {
		Expr::ExprUnary(node)
	}
}
impl From<ExprSlice> for Expr {
	fn from(node: ExprSlice) -> Expr {
		Expr::ExprSlice(node)
	}
}
impl From<ExprIndex> for Expr {
	fn from(node: ExprIndex) -> Expr {
		Expr::ExprIndex(node)
	}
}
impl From<ExprIndexExpr> for Expr {
	fn from(node: ExprIndexExpr) -> Expr {
		Expr::ExprIndexExpr(node)
	}
}
impl From<ExprApply> for Expr {
	fn from(node: ExprApply) -> Expr {
		Expr::ExprApply(node)
	}
}
impl From<ExprObjExtend> for Expr {
	fn from(node: ExprObjExtend) -> Expr {
		Expr::ExprObjExtend(node)
	}
}
impl From<ExprParened> for Expr {
	fn from(node: ExprParened) -> Expr {
		Expr::ExprParened(node)
	}
}
impl From<ExprIntrinsicThisFile> for Expr {
	fn from(node: ExprIntrinsicThisFile) -> Expr {
		Expr::ExprIntrinsicThisFile(node)
	}
}
impl From<ExprIntrinsicId> for Expr {
	fn from(node: ExprIntrinsicId) -> Expr {
		Expr::ExprIntrinsicId(node)
	}
}
impl From<ExprIntrinsic> for Expr {
	fn from(node: ExprIntrinsic) -> Expr {
		Expr::ExprIntrinsic(node)
	}
}
impl From<ExprString> for Expr {
	fn from(node: ExprString) -> Expr {
		Expr::ExprString(node)
	}
}
impl From<ExprNumber> for Expr {
	fn from(node: ExprNumber) -> Expr {
		Expr::ExprNumber(node)
	}
}
impl From<ExprLiteral> for Expr {
	fn from(node: ExprLiteral) -> Expr {
		Expr::ExprLiteral(node)
	}
}
impl From<ExprArray> for Expr {
	fn from(node: ExprArray) -> Expr {
		Expr::ExprArray(node)
	}
}
impl From<ExprObject> for Expr {
	fn from(node: ExprObject) -> Expr {
		Expr::ExprObject(node)
	}
}
impl From<ExprArrayComp> for Expr {
	fn from(node: ExprArrayComp) -> Expr {
		Expr::ExprArrayComp(node)
	}
}
impl From<ExprImport> for Expr {
	fn from(node: ExprImport) -> Expr {
		Expr::ExprImport(node)
	}
}
impl From<ExprVar> for Expr {
	fn from(node: ExprVar) -> Expr {
		Expr::ExprVar(node)
	}
}
impl From<ExprLocal> for Expr {
	fn from(node: ExprLocal) -> Expr {
		Expr::ExprLocal(node)
	}
}
impl From<ExprIfThenElse> for Expr {
	fn from(node: ExprIfThenElse) -> Expr {
		Expr::ExprIfThenElse(node)
	}
}
impl From<ExprFunction> for Expr {
	fn from(node: ExprFunction) -> Expr {
		Expr::ExprFunction(node)
	}
}
impl From<ExprAssert> for Expr {
	fn from(node: ExprAssert) -> Expr {
		Expr::ExprAssert(node)
	}
}
impl From<ExprError> for Expr {
	fn from(node: ExprError) -> Expr {
		Expr::ExprError(node)
	}
}
impl AstNode for Expr {
	fn can_cast(kind: SyntaxKind) -> bool {
		match kind {
			EXPR_BINARY
			| EXPR_UNARY
			| EXPR_SLICE
			| EXPR_INDEX
			| EXPR_INDEX_EXPR
			| EXPR_APPLY
			| EXPR_OBJ_EXTEND
			| EXPR_PARENED
			| EXPR_INTRINSIC_THIS_FILE
			| EXPR_INTRINSIC_ID
			| EXPR_INTRINSIC
			| EXPR_STRING
			| EXPR_NUMBER
			| EXPR_LITERAL
			| EXPR_ARRAY
			| EXPR_OBJECT
			| EXPR_ARRAY_COMP
			| EXPR_IMPORT
			| EXPR_VAR
			| EXPR_LOCAL
			| EXPR_IF_THEN_ELSE
			| EXPR_FUNCTION
			| EXPR_ASSERT
			| EXPR_ERROR => true,
			_ => false,
		}
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		let res = match syntax.kind() {
			EXPR_BINARY => Expr::ExprBinary(ExprBinary { syntax }),
			EXPR_UNARY => Expr::ExprUnary(ExprUnary { syntax }),
			EXPR_SLICE => Expr::ExprSlice(ExprSlice { syntax }),
			EXPR_INDEX => Expr::ExprIndex(ExprIndex { syntax }),
			EXPR_INDEX_EXPR => Expr::ExprIndexExpr(ExprIndexExpr { syntax }),
			EXPR_APPLY => Expr::ExprApply(ExprApply { syntax }),
			EXPR_OBJ_EXTEND => Expr::ExprObjExtend(ExprObjExtend { syntax }),
			EXPR_PARENED => Expr::ExprParened(ExprParened { syntax }),
			EXPR_INTRINSIC_THIS_FILE => {
				Expr::ExprIntrinsicThisFile(ExprIntrinsicThisFile { syntax })
			}
			EXPR_INTRINSIC_ID => Expr::ExprIntrinsicId(ExprIntrinsicId { syntax }),
			EXPR_INTRINSIC => Expr::ExprIntrinsic(ExprIntrinsic { syntax }),
			EXPR_STRING => Expr::ExprString(ExprString { syntax }),
			EXPR_NUMBER => Expr::ExprNumber(ExprNumber { syntax }),
			EXPR_LITERAL => Expr::ExprLiteral(ExprLiteral { syntax }),
			EXPR_ARRAY => Expr::ExprArray(ExprArray { syntax }),
			EXPR_OBJECT => Expr::ExprObject(ExprObject { syntax }),
			EXPR_ARRAY_COMP => Expr::ExprArrayComp(ExprArrayComp { syntax }),
			EXPR_IMPORT => Expr::ExprImport(ExprImport { syntax }),
			EXPR_VAR => Expr::ExprVar(ExprVar { syntax }),
			EXPR_LOCAL => Expr::ExprLocal(ExprLocal { syntax }),
			EXPR_IF_THEN_ELSE => Expr::ExprIfThenElse(ExprIfThenElse { syntax }),
			EXPR_FUNCTION => Expr::ExprFunction(ExprFunction { syntax }),
			EXPR_ASSERT => Expr::ExprAssert(ExprAssert { syntax }),
			EXPR_ERROR => Expr::ExprError(ExprError { syntax }),
			_ => return None,
		};
		Some(res)
	}
	fn syntax(&self) -> &SyntaxNode {
		match self {
			Expr::ExprBinary(it) => &it.syntax,
			Expr::ExprUnary(it) => &it.syntax,
			Expr::ExprSlice(it) => &it.syntax,
			Expr::ExprIndex(it) => &it.syntax,
			Expr::ExprIndexExpr(it) => &it.syntax,
			Expr::ExprApply(it) => &it.syntax,
			Expr::ExprObjExtend(it) => &it.syntax,
			Expr::ExprParened(it) => &it.syntax,
			Expr::ExprIntrinsicThisFile(it) => &it.syntax,
			Expr::ExprIntrinsicId(it) => &it.syntax,
			Expr::ExprIntrinsic(it) => &it.syntax,
			Expr::ExprString(it) => &it.syntax,
			Expr::ExprNumber(it) => &it.syntax,
			Expr::ExprLiteral(it) => &it.syntax,
			Expr::ExprArray(it) => &it.syntax,
			Expr::ExprObject(it) => &it.syntax,
			Expr::ExprArrayComp(it) => &it.syntax,
			Expr::ExprImport(it) => &it.syntax,
			Expr::ExprVar(it) => &it.syntax,
			Expr::ExprLocal(it) => &it.syntax,
			Expr::ExprIfThenElse(it) => &it.syntax,
			Expr::ExprFunction(it) => &it.syntax,
			Expr::ExprAssert(it) => &it.syntax,
			Expr::ExprError(it) => &it.syntax,
		}
	}
}
impl From<ObjBodyComp> for ObjBody {
	fn from(node: ObjBodyComp) -> ObjBody {
		ObjBody::ObjBodyComp(node)
	}
}
impl From<ObjBodyMemberList> for ObjBody {
	fn from(node: ObjBodyMemberList) -> ObjBody {
		ObjBody::ObjBodyMemberList(node)
	}
}
impl AstNode for ObjBody {
	fn can_cast(kind: SyntaxKind) -> bool {
		match kind {
			OBJ_BODY_COMP | OBJ_BODY_MEMBER_LIST => true,
			_ => false,
		}
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		let res = match syntax.kind() {
			OBJ_BODY_COMP => ObjBody::ObjBodyComp(ObjBodyComp { syntax }),
			OBJ_BODY_MEMBER_LIST => ObjBody::ObjBodyMemberList(ObjBodyMemberList { syntax }),
			_ => return None,
		};
		Some(res)
	}
	fn syntax(&self) -> &SyntaxNode {
		match self {
			ObjBody::ObjBodyComp(it) => &it.syntax,
			ObjBody::ObjBodyMemberList(it) => &it.syntax,
		}
	}
}
impl From<ForSpec> for CompSpec {
	fn from(node: ForSpec) -> CompSpec {
		CompSpec::ForSpec(node)
	}
}
impl From<IfSpec> for CompSpec {
	fn from(node: IfSpec) -> CompSpec {
		CompSpec::IfSpec(node)
	}
}
impl AstNode for CompSpec {
	fn can_cast(kind: SyntaxKind) -> bool {
		match kind {
			FOR_SPEC | IF_SPEC => true,
			_ => false,
		}
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		let res = match syntax.kind() {
			FOR_SPEC => CompSpec::ForSpec(ForSpec { syntax }),
			IF_SPEC => CompSpec::IfSpec(IfSpec { syntax }),
			_ => return None,
		};
		Some(res)
	}
	fn syntax(&self) -> &SyntaxNode {
		match self {
			CompSpec::ForSpec(it) => &it.syntax,
			CompSpec::IfSpec(it) => &it.syntax,
		}
	}
}
impl From<BindDestruct> for Bind {
	fn from(node: BindDestruct) -> Bind {
		Bind::BindDestruct(node)
	}
}
impl From<BindFunction> for Bind {
	fn from(node: BindFunction) -> Bind {
		Bind::BindFunction(node)
	}
}
impl AstNode for Bind {
	fn can_cast(kind: SyntaxKind) -> bool {
		match kind {
			BIND_DESTRUCT | BIND_FUNCTION => true,
			_ => false,
		}
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		let res = match syntax.kind() {
			BIND_DESTRUCT => Bind::BindDestruct(BindDestruct { syntax }),
			BIND_FUNCTION => Bind::BindFunction(BindFunction { syntax }),
			_ => return None,
		};
		Some(res)
	}
	fn syntax(&self) -> &SyntaxNode {
		match self {
			Bind::BindDestruct(it) => &it.syntax,
			Bind::BindFunction(it) => &it.syntax,
		}
	}
}
impl From<MemberBindStmt> for Member {
	fn from(node: MemberBindStmt) -> Member {
		Member::MemberBindStmt(node)
	}
}
impl From<MemberAssertStmt> for Member {
	fn from(node: MemberAssertStmt) -> Member {
		Member::MemberAssertStmt(node)
	}
}
impl From<MemberField> for Member {
	fn from(node: MemberField) -> Member {
		Member::MemberField(node)
	}
}
impl AstNode for Member {
	fn can_cast(kind: SyntaxKind) -> bool {
		match kind {
			MEMBER_BIND_STMT | MEMBER_ASSERT_STMT | MEMBER_FIELD => true,
			_ => false,
		}
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		let res = match syntax.kind() {
			MEMBER_BIND_STMT => Member::MemberBindStmt(MemberBindStmt { syntax }),
			MEMBER_ASSERT_STMT => Member::MemberAssertStmt(MemberAssertStmt { syntax }),
			MEMBER_FIELD => Member::MemberField(MemberField { syntax }),
			_ => return None,
		};
		Some(res)
	}
	fn syntax(&self) -> &SyntaxNode {
		match self {
			Member::MemberBindStmt(it) => &it.syntax,
			Member::MemberAssertStmt(it) => &it.syntax,
			Member::MemberField(it) => &it.syntax,
		}
	}
}
impl From<FieldNormal> for Field {
	fn from(node: FieldNormal) -> Field {
		Field::FieldNormal(node)
	}
}
impl From<FieldMethod> for Field {
	fn from(node: FieldMethod) -> Field {
		Field::FieldMethod(node)
	}
}
impl AstNode for Field {
	fn can_cast(kind: SyntaxKind) -> bool {
		match kind {
			FIELD_NORMAL | FIELD_METHOD => true,
			_ => false,
		}
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		let res = match syntax.kind() {
			FIELD_NORMAL => Field::FieldNormal(FieldNormal { syntax }),
			FIELD_METHOD => Field::FieldMethod(FieldMethod { syntax }),
			_ => return None,
		};
		Some(res)
	}
	fn syntax(&self) -> &SyntaxNode {
		match self {
			Field::FieldNormal(it) => &it.syntax,
			Field::FieldMethod(it) => &it.syntax,
		}
	}
}
impl From<FieldNameFixed> for FieldName {
	fn from(node: FieldNameFixed) -> FieldName {
		FieldName::FieldNameFixed(node)
	}
}
impl From<FieldNameDynamic> for FieldName {
	fn from(node: FieldNameDynamic) -> FieldName {
		FieldName::FieldNameDynamic(node)
	}
}
impl AstNode for FieldName {
	fn can_cast(kind: SyntaxKind) -> bool {
		match kind {
			FIELD_NAME_FIXED | FIELD_NAME_DYNAMIC => true,
			_ => false,
		}
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		let res = match syntax.kind() {
			FIELD_NAME_FIXED => FieldName::FieldNameFixed(FieldNameFixed { syntax }),
			FIELD_NAME_DYNAMIC => FieldName::FieldNameDynamic(FieldNameDynamic { syntax }),
			_ => return None,
		};
		Some(res)
	}
	fn syntax(&self) -> &SyntaxNode {
		match self {
			FieldName::FieldNameFixed(it) => &it.syntax,
			FieldName::FieldNameDynamic(it) => &it.syntax,
		}
	}
}
impl From<DestructFull> for Destruct {
	fn from(node: DestructFull) -> Destruct {
		Destruct::DestructFull(node)
	}
}
impl From<DestructSkip> for Destruct {
	fn from(node: DestructSkip) -> Destruct {
		Destruct::DestructSkip(node)
	}
}
impl From<DestructArray> for Destruct {
	fn from(node: DestructArray) -> Destruct {
		Destruct::DestructArray(node)
	}
}
impl From<DestructObject> for Destruct {
	fn from(node: DestructObject) -> Destruct {
		Destruct::DestructObject(node)
	}
}
impl AstNode for Destruct {
	fn can_cast(kind: SyntaxKind) -> bool {
		match kind {
			DESTRUCT_FULL | DESTRUCT_SKIP | DESTRUCT_ARRAY | DESTRUCT_OBJECT => true,
			_ => false,
		}
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		let res = match syntax.kind() {
			DESTRUCT_FULL => Destruct::DestructFull(DestructFull { syntax }),
			DESTRUCT_SKIP => Destruct::DestructSkip(DestructSkip { syntax }),
			DESTRUCT_ARRAY => Destruct::DestructArray(DestructArray { syntax }),
			DESTRUCT_OBJECT => Destruct::DestructObject(DestructObject { syntax }),
			_ => return None,
		};
		Some(res)
	}
	fn syntax(&self) -> &SyntaxNode {
		match self {
			Destruct::DestructFull(it) => &it.syntax,
			Destruct::DestructSkip(it) => &it.syntax,
			Destruct::DestructArray(it) => &it.syntax,
			Destruct::DestructObject(it) => &it.syntax,
		}
	}
}
impl From<DestructArrayElement> for DestructArrayPart {
	fn from(node: DestructArrayElement) -> DestructArrayPart {
		DestructArrayPart::DestructArrayElement(node)
	}
}
impl From<DestructRest> for DestructArrayPart {
	fn from(node: DestructRest) -> DestructArrayPart {
		DestructArrayPart::DestructRest(node)
	}
}
impl AstNode for DestructArrayPart {
	fn can_cast(kind: SyntaxKind) -> bool {
		match kind {
			DESTRUCT_ARRAY_ELEMENT | DESTRUCT_REST => true,
			_ => false,
		}
	}
	fn cast(syntax: SyntaxNode) -> Option<Self> {
		let res = match syntax.kind() {
			DESTRUCT_ARRAY_ELEMENT => {
				DestructArrayPart::DestructArrayElement(DestructArrayElement { syntax })
			}
			DESTRUCT_REST => DestructArrayPart::DestructRest(DestructRest { syntax }),
			_ => return None,
		};
		Some(res)
	}
	fn syntax(&self) -> &SyntaxNode {
		match self {
			DestructArrayPart::DestructArrayElement(it) => &it.syntax,
			DestructArrayPart::DestructRest(it) => &it.syntax,
		}
	}
}
impl AstToken for BinaryOperator {
	fn can_cast(kind: SyntaxKind) -> bool {
		match kind {
			OR | AND | BIT_OR | BIT_XOR | BIT_AND | EQ | NE | LT | GT | LE | GE | IN_KW | LHS
			| RHS | PLUS | MINUS | MUL | DIV | MODULO | ERROR_NO_OPERATOR => true,
			_ => false,
		}
	}
	fn cast(syntax: SyntaxToken) -> Option<Self> {
		let res = match syntax.kind() {
			OR => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::Or,
			},
			AND => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::And,
			},
			BIT_OR => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::BitOr,
			},
			BIT_XOR => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::BitXor,
			},
			BIT_AND => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::BitAnd,
			},
			EQ => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::Eq,
			},
			NE => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::Ne,
			},
			LT => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::Lt,
			},
			GT => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::Gt,
			},
			LE => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::Le,
			},
			GE => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::Ge,
			},
			IN_KW => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::InKw,
			},
			LHS => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::Lhs,
			},
			RHS => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::Rhs,
			},
			PLUS => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::Plus,
			},
			MINUS => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::Minus,
			},
			MUL => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::Mul,
			},
			DIV => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::Div,
			},
			MODULO => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::Modulo,
			},
			ERROR_NO_OPERATOR => BinaryOperator {
				syntax,
				kind: BinaryOperatorKind::ErrorNoOperator,
			},
			_ => return None,
		};
		Some(res)
	}
	fn syntax(&self) -> &SyntaxToken {
		&self.syntax
	}
}
impl BinaryOperator {
	pub fn kind(&self) -> BinaryOperatorKind {
		self.kind
	}
}
impl std::fmt::Display for BinaryOperator {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl AstToken for UnaryOperator {
	fn can_cast(kind: SyntaxKind) -> bool {
		match kind {
			MINUS | NOT | BIT_NOT => true,
			_ => false,
		}
	}
	fn cast(syntax: SyntaxToken) -> Option<Self> {
		let res = match syntax.kind() {
			MINUS => UnaryOperator {
				syntax,
				kind: UnaryOperatorKind::Minus,
			},
			NOT => UnaryOperator {
				syntax,
				kind: UnaryOperatorKind::Not,
			},
			BIT_NOT => UnaryOperator {
				syntax,
				kind: UnaryOperatorKind::BitNot,
			},
			_ => return None,
		};
		Some(res)
	}
	fn syntax(&self) -> &SyntaxToken {
		&self.syntax
	}
}
impl UnaryOperator {
	pub fn kind(&self) -> UnaryOperatorKind {
		self.kind
	}
}
impl std::fmt::Display for UnaryOperator {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl AstToken for Literal {
	fn can_cast(kind: SyntaxKind) -> bool {
		match kind {
			NULL_KW | TRUE_KW | FALSE_KW | SELF_KW | DOLLAR | SUPER_KW => true,
			_ => false,
		}
	}
	fn cast(syntax: SyntaxToken) -> Option<Self> {
		let res = match syntax.kind() {
			NULL_KW => Literal {
				syntax,
				kind: LiteralKind::NullKw,
			},
			TRUE_KW => Literal {
				syntax,
				kind: LiteralKind::TrueKw,
			},
			FALSE_KW => Literal {
				syntax,
				kind: LiteralKind::FalseKw,
			},
			SELF_KW => Literal {
				syntax,
				kind: LiteralKind::SelfKw,
			},
			DOLLAR => Literal {
				syntax,
				kind: LiteralKind::Dollar,
			},
			SUPER_KW => Literal {
				syntax,
				kind: LiteralKind::SuperKw,
			},
			_ => return None,
		};
		Some(res)
	}
	fn syntax(&self) -> &SyntaxToken {
		&self.syntax
	}
}
impl Literal {
	pub fn kind(&self) -> LiteralKind {
		self.kind
	}
}
impl std::fmt::Display for Literal {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl AstToken for Text {
	fn can_cast(kind: SyntaxKind) -> bool {
		match kind {
			STRING_DOUBLE
			| ERROR_STRING_DOUBLE_UNTERMINATED
			| STRING_SINGLE
			| ERROR_STRING_SINGLE_UNTERMINATED
			| STRING_DOUBLE_VERBATIM
			| ERROR_STRING_DOUBLE_VERBATIM_UNTERMINATED
			| STRING_SINGLE_VERBATIM
			| ERROR_STRING_SINGLE_VERBATIM_UNTERMINATED
			| ERROR_STRING_VERBATIM_MISSING_QUOTES
			| STRING_BLOCK
			| ERROR_STRING_BLOCK_UNEXPECTED_END
			| ERROR_STRING_BLOCK_MISSING_NEW_LINE
			| ERROR_STRING_BLOCK_MISSING_TERMINATION
			| ERROR_STRING_BLOCK_MISSING_INDENT => true,
			_ => false,
		}
	}
	fn cast(syntax: SyntaxToken) -> Option<Self> {
		let res = match syntax.kind() {
			STRING_DOUBLE => Text {
				syntax,
				kind: TextKind::StringDouble,
			},
			ERROR_STRING_DOUBLE_UNTERMINATED => Text {
				syntax,
				kind: TextKind::ErrorStringDoubleUnterminated,
			},
			STRING_SINGLE => Text {
				syntax,
				kind: TextKind::StringSingle,
			},
			ERROR_STRING_SINGLE_UNTERMINATED => Text {
				syntax,
				kind: TextKind::ErrorStringSingleUnterminated,
			},
			STRING_DOUBLE_VERBATIM => Text {
				syntax,
				kind: TextKind::StringDoubleVerbatim,
			},
			ERROR_STRING_DOUBLE_VERBATIM_UNTERMINATED => Text {
				syntax,
				kind: TextKind::ErrorStringDoubleVerbatimUnterminated,
			},
			STRING_SINGLE_VERBATIM => Text {
				syntax,
				kind: TextKind::StringSingleVerbatim,
			},
			ERROR_STRING_SINGLE_VERBATIM_UNTERMINATED => Text {
				syntax,
				kind: TextKind::ErrorStringSingleVerbatimUnterminated,
			},
			ERROR_STRING_VERBATIM_MISSING_QUOTES => Text {
				syntax,
				kind: TextKind::ErrorStringVerbatimMissingQuotes,
			},
			STRING_BLOCK => Text {
				syntax,
				kind: TextKind::StringBlock,
			},
			ERROR_STRING_BLOCK_UNEXPECTED_END => Text {
				syntax,
				kind: TextKind::ErrorStringBlockUnexpectedEnd,
			},
			ERROR_STRING_BLOCK_MISSING_NEW_LINE => Text {
				syntax,
				kind: TextKind::ErrorStringBlockMissingNewLine,
			},
			ERROR_STRING_BLOCK_MISSING_TERMINATION => Text {
				syntax,
				kind: TextKind::ErrorStringBlockMissingTermination,
			},
			ERROR_STRING_BLOCK_MISSING_INDENT => Text {
				syntax,
				kind: TextKind::ErrorStringBlockMissingIndent,
			},
			_ => return None,
		};
		Some(res)
	}
	fn syntax(&self) -> &SyntaxToken {
		&self.syntax
	}
}
impl Text {
	pub fn kind(&self) -> TextKind {
		self.kind
	}
}
impl std::fmt::Display for Text {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl AstToken for Number {
	fn can_cast(kind: SyntaxKind) -> bool {
		match kind {
			FLOAT
			| ERROR_FLOAT_JUNK_AFTER_POINT
			| ERROR_FLOAT_JUNK_AFTER_EXPONENT
			| ERROR_FLOAT_JUNK_AFTER_EXPONENT_SIGN => true,
			_ => false,
		}
	}
	fn cast(syntax: SyntaxToken) -> Option<Self> {
		let res = match syntax.kind() {
			FLOAT => Number {
				syntax,
				kind: NumberKind::Float,
			},
			ERROR_FLOAT_JUNK_AFTER_POINT => Number {
				syntax,
				kind: NumberKind::ErrorFloatJunkAfterPoint,
			},
			ERROR_FLOAT_JUNK_AFTER_EXPONENT => Number {
				syntax,
				kind: NumberKind::ErrorFloatJunkAfterExponent,
			},
			ERROR_FLOAT_JUNK_AFTER_EXPONENT_SIGN => Number {
				syntax,
				kind: NumberKind::ErrorFloatJunkAfterExponentSign,
			},
			_ => return None,
		};
		Some(res)
	}
	fn syntax(&self) -> &SyntaxToken {
		&self.syntax
	}
}
impl Number {
	pub fn kind(&self) -> NumberKind {
		self.kind
	}
}
impl std::fmt::Display for Number {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl AstToken for ImportKind {
	fn can_cast(kind: SyntaxKind) -> bool {
		match kind {
			IMPORTSTR_KW | IMPORTBIN_KW | IMPORT_KW => true,
			_ => false,
		}
	}
	fn cast(syntax: SyntaxToken) -> Option<Self> {
		let res = match syntax.kind() {
			IMPORTSTR_KW => ImportKind {
				syntax,
				kind: ImportKindKind::ImportstrKw,
			},
			IMPORTBIN_KW => ImportKind {
				syntax,
				kind: ImportKindKind::ImportbinKw,
			},
			IMPORT_KW => ImportKind {
				syntax,
				kind: ImportKindKind::ImportKw,
			},
			_ => return None,
		};
		Some(res)
	}
	fn syntax(&self) -> &SyntaxToken {
		&self.syntax
	}
}
impl ImportKind {
	pub fn kind(&self) -> ImportKindKind {
		self.kind
	}
}
impl std::fmt::Display for ImportKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl AstToken for Visibility {
	fn can_cast(kind: SyntaxKind) -> bool {
		match kind {
			COLONCOLONCOLON | COLONCOLON | COLON => true,
			_ => false,
		}
	}
	fn cast(syntax: SyntaxToken) -> Option<Self> {
		let res = match syntax.kind() {
			COLONCOLONCOLON => Visibility {
				syntax,
				kind: VisibilityKind::Coloncoloncolon,
			},
			COLONCOLON => Visibility {
				syntax,
				kind: VisibilityKind::Coloncolon,
			},
			COLON => Visibility {
				syntax,
				kind: VisibilityKind::Colon,
			},
			_ => return None,
		};
		Some(res)
	}
	fn syntax(&self) -> &SyntaxToken {
		&self.syntax
	}
}
impl Visibility {
	pub fn kind(&self) -> VisibilityKind {
		self.kind
	}
}
impl std::fmt::Display for Visibility {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl AstToken for Trivia {
	fn can_cast(kind: SyntaxKind) -> bool {
		match kind {
			WHITESPACE
			| MULTI_LINE_COMMENT
			| ERROR_COMMENT_TOO_SHORT
			| ERROR_COMMENT_UNTERMINATED
			| SINGLE_LINE_HASH_COMMENT
			| SINGLE_LINE_SLASH_COMMENT => true,
			_ => false,
		}
	}
	fn cast(syntax: SyntaxToken) -> Option<Self> {
		let res = match syntax.kind() {
			WHITESPACE => Trivia {
				syntax,
				kind: TriviaKind::Whitespace,
			},
			MULTI_LINE_COMMENT => Trivia {
				syntax,
				kind: TriviaKind::MultiLineComment,
			},
			ERROR_COMMENT_TOO_SHORT => Trivia {
				syntax,
				kind: TriviaKind::ErrorCommentTooShort,
			},
			ERROR_COMMENT_UNTERMINATED => Trivia {
				syntax,
				kind: TriviaKind::ErrorCommentUnterminated,
			},
			SINGLE_LINE_HASH_COMMENT => Trivia {
				syntax,
				kind: TriviaKind::SingleLineHashComment,
			},
			SINGLE_LINE_SLASH_COMMENT => Trivia {
				syntax,
				kind: TriviaKind::SingleLineSlashComment,
			},
			_ => return None,
		};
		Some(res)
	}
	fn syntax(&self) -> &SyntaxToken {
		&self.syntax
	}
}
impl Trivia {
	pub fn kind(&self) -> TriviaKind {
		self.kind
	}
}
impl std::fmt::Display for Trivia {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for Expr {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ObjBody {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for CompSpec {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for Bind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for Member {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for Field {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for FieldName {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for Destruct {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for DestructArrayPart {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for SourceFile {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprBinary {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for LhsExpr {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprUnary {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprSlice {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for SliceDesc {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprIndex {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for Name {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprIndexExpr {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprApply {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ArgsDesc {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprObjExtend {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprParened {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprLiteral {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprIntrinsicThisFile {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprIntrinsicId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprIntrinsic {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprString {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprNumber {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprArray {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprObject {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprArrayComp {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprImport {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprVar {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprLocal {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprIfThenElse {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for TrueExpr {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for FalseExpr {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprFunction {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ParamsDesc {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprAssert {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for Assertion {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ExprError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for SliceDescEnd {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for SliceDescStep {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for Arg {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ObjBodyComp {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ObjLocalPostComma {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ObjLocalPreComma {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ObjBodyMemberList {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ObjLocal {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for MemberBindStmt {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for MemberAssertStmt {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for MemberField {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for FieldNormal {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for FieldMethod {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for FieldNameFixed {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for FieldNameDynamic {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for ForSpec {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for IfSpec {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for BindDestruct {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for BindFunction {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for Param {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for DestructFull {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for DestructSkip {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for DestructArray {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for DestructObject {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for DestructObjectField {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for DestructRest {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
impl std::fmt::Display for DestructArrayElement {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		std::fmt::Display::fmt(self.syntax(), f)
	}
}
