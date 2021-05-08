use crate::ident::Ident;
use std::fmt;
use std::rc::Rc;
use makepad_live_parser::Span;
use makepad_live_parser::Id;
use makepad_live_parser::id;
use std::cell::{RefCell};
use crate::shaderast::TyExpr;
use crate::shaderast::TyExprKind;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Ord, PartialOrd)]
pub enum Ty {
    Void,
    Bool,
    Int,
    Float,
    Bvec2,
    Bvec3,
    Bvec4,
    Ivec2,
    Ivec3,
    Ivec4,
    Vec2,
    Vec3,
    Vec4,
    Mat2,
    Mat3,
    Mat4,
    Texture2D,
    Array { elem_ty: Rc<Ty>, len: usize },
    Struct { ident: Ident },
}

impl Ty {
    
    pub fn maybe_ty_lit(&self)->Option<TyLit>{
        match self{
            Ty::Void => None,
            Ty::Bool => Some(TyLit::Bool),
            Ty::Int =>  Some(TyLit::Int),
            Ty::Float => Some(TyLit::Float),
            Ty::Bvec2 => Some(TyLit::Bvec2),
            Ty::Bvec3 => Some(TyLit::Bvec3),
            Ty::Bvec4 => Some(TyLit::Bvec4),
            Ty::Ivec2 => Some(TyLit::Ivec2),
            Ty::Ivec3 => Some(TyLit::Ivec3),
            Ty::Ivec4 => Some(TyLit::Ivec4),
            Ty::Vec2 => Some(TyLit::Vec2),
            Ty::Vec3 => Some(TyLit::Vec3),
            Ty::Vec4 => Some(TyLit::Vec4),
            Ty::Mat2 => Some(TyLit::Mat2),
            Ty::Mat3 => Some(TyLit::Mat3),
            Ty::Mat4 => Some(TyLit::Mat4),
            Ty::Texture2D => Some(TyLit::Bool),
            Ty::Array { .. } => None,
            Ty::Struct { .. } => None
        }
    }
    
    pub fn is_scalar(&self) -> bool {
        match self {
            Ty::Bool | Ty::Int | Ty::Float => true,
            _ => false,
        }
    }

    pub fn is_vector(&self) -> bool {
        match self {
            Ty::Bvec2
            | Ty::Bvec3
            | Ty::Bvec4
            | Ty::Ivec2
            | Ty::Ivec3
            | Ty::Ivec4
            | Ty::Vec2
            | Ty::Vec3
            | Ty::Vec4 => true,
            _ => false,
        }
    }

    pub fn is_matrix(&self) -> bool {
        match self {
            Ty::Mat2 | Ty::Mat3 | Ty::Mat4 => true,
            _ => false,
        }
    }

    pub fn size(&self) -> usize {
        match self {
            Ty::Void => 0,
            Ty::Bool | Ty::Int | Ty::Float => 1,
            Ty::Bvec2 | Ty::Ivec2 | Ty::Vec2 => 2,
            Ty::Bvec3 | Ty::Ivec3 | Ty::Vec3 => 3,
            Ty::Bvec4 | Ty::Ivec4 | Ty::Vec4 | Ty::Mat2 => 4,
            Ty::Mat3 => 9,
            Ty::Mat4 => 16,
            Ty::Texture2D { .. } => panic!(),
            Ty::Array { elem_ty, len } => elem_ty.size() * len,
            Ty::Struct { .. } => panic!(),
        }
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Ty::Void => write!(f, "void"),
            Ty::Bool => write!(f, "bool"),
            Ty::Int => write!(f, "int"),
            Ty::Float => write!(f, "float"),
            Ty::Bvec2 => write!(f, "bvec2"),
            Ty::Bvec3 => write!(f, "bvec3"),
            Ty::Bvec4 => write!(f, "bvec4"),
            Ty::Ivec2 => write!(f, "ivec2"),
            Ty::Ivec3 => write!(f, "ivec3"),
            Ty::Ivec4 => write!(f, "ivec4"),
            Ty::Vec2 => write!(f, "vec2"),
            Ty::Vec3 => write!(f, "vec3"),
            Ty::Vec4 => write!(f, "vec4"),
            Ty::Mat2 => write!(f, "mat2"),
            Ty::Mat3 => write!(f, "mat3"),
            Ty::Mat4 => write!(f, "mat4"),
            Ty::Texture2D => write!(f, "texture2D"),
            Ty::Array { elem_ty, len } => write!(f, "{}[{}]", elem_ty, len),
            Ty::Struct { ident, .. } => write!(f, "{}", ident),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Ord, PartialOrd)]
pub enum TyLit {
    Bool,
    Int,
    Float,
    Bvec2,
    Bvec3,
    Bvec4,
    Ivec2,
    Ivec3,
    Ivec4,
    Vec2,
    Vec3,
    Vec4,
    Mat2,
    Mat3,
    Mat4,
    Texture2D,
}

impl TyLit {
    pub fn from_id(id:Id)->Option<TyLit>{
        match id{
            id!(vec4)=>Some(TyLit::Vec4),
            id!(vec3)=>Some(TyLit::Vec3),
            id!(vec2)=>Some(TyLit::Vec2),
            id!(mat4)=>Some(TyLit::Mat4),
            id!(mat3)=>Some(TyLit::Mat3),
            id!(mat2)=>Some(TyLit::Mat2),
            id!(float)=>Some(TyLit::Float),
            id!(bool)=>Some(TyLit::Bool),
            id!(int)=>Some(TyLit::Int),
            id!(bvec2)=>Some(TyLit::Bvec2),
            id!(bvec3)=>Some(TyLit::Bvec3),
            id!(bvec4)=>Some(TyLit::Bvec4),
            id!(ivec2)=>Some(TyLit::Ivec4),
            id!(ivec3)=>Some(TyLit::Ivec4),
            id!(ivec4)=>Some(TyLit::Ivec4),
            id!(texture2D)=>Some(TyLit::Texture2D),
            _=>None
        }
    }
    
    pub fn to_ty_expr(self) ->TyExpr{
        TyExpr {
            ty: RefCell::new(None),
            kind: TyExprKind::Lit {
                span: Span::default(),
                ty_lit: self
            }
        }
    }
    
    pub fn to_ty(self) -> Ty {
        match self {
            TyLit::Bool => Ty::Bool,
            TyLit::Int => Ty::Int,
            TyLit::Float => Ty::Float,
            TyLit::Bvec2 => Ty::Bvec2,
            TyLit::Bvec3 => Ty::Bvec3,
            TyLit::Bvec4 => Ty::Bvec4,
            TyLit::Ivec2 => Ty::Ivec2,
            TyLit::Ivec3 => Ty::Ivec3,
            TyLit::Ivec4 => Ty::Ivec4,
            TyLit::Vec2 => Ty::Vec2,
            TyLit::Vec3 => Ty::Vec3,
            TyLit::Vec4 => Ty::Vec4,
            TyLit::Mat2 => Ty::Mat2,
            TyLit::Mat3 => Ty::Mat3,
            TyLit::Mat4 => Ty::Mat4,
            TyLit::Texture2D => Ty::Texture2D,
        }
    }
    
}

impl fmt::Display for TyLit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TyLit::Bool => "bool",
                TyLit::Int => "int",
                TyLit::Float => "float",
                TyLit::Bvec2 => "bvec2",
                TyLit::Bvec3 => "bvec3",
                TyLit::Bvec4 => "bvec4",
                TyLit::Ivec2 => "ivec2",
                TyLit::Ivec3 => "ivec3",
                TyLit::Ivec4 => "ivec4",
                TyLit::Vec2 => "vec2",
                TyLit::Vec3 => "vec3",
                TyLit::Vec4 => "vec4",
                TyLit::Mat2 => "mat2",
                TyLit::Mat3 => "mat3",
                TyLit::Mat4 => "mat4",
                TyLit::Texture2D => "texture2D",
            }
        )
    }
}


