use crate::{
    ast::{AstNode, ModuleAst},
    irgen::{IRGen, IRGenErr},
    parser::{IRParseErr, IRParser},
};
use remusys_ir::{ir::Module, typing::ArchInfo};

pub mod ast;
pub mod irgen;
pub mod mapping;
pub mod parser;
pub mod sema;
pub mod tokens;

#[derive(Debug, thiserror::Error)]
pub enum CompileErr {
    #[error("parser error: {} at {:?}", _0.kind, _0.span)]
    Parse(IRParseErr),

    #[error("{0}")]
    IRGen(#[from] IRGenErr),
}
impl From<IRParseErr> for CompileErr {
    fn from(value: IRParseErr) -> Self {
        CompileErr::Parse(value)
    }
}

impl CompileErr {
    pub fn get_span(&self) -> logos::Span {
        match self {
            CompileErr::Parse(e) => e.span.clone(),
            CompileErr::IRGen(e) => e.span.clone(),
        }
    }

    pub fn get_lines_source<'a>(&self, source: &'a str, line_poses: &[usize]) -> &'a str {
        use logos::Span;
        let Span {
            start: start_off,
            end: end_off,
        } = self.get_span();
        let start_line = match line_poses.binary_search(&start_off) {
            Ok(line) => line,
            Err(line) => line.saturating_sub(1),
        };
        let end_line = match line_poses.binary_search(&end_off) {
            Ok(line) => line,
            Err(line) => line.saturating_sub(1),
        };
        let start_pos = line_poses.get(start_line).copied().unwrap_or(0);
        let end_pos = line_poses
            .get(end_line + 1)
            .copied()
            .unwrap_or(source.len());
        &source[start_pos..end_pos]
    }

    pub fn dump_string(&self, source: &str, line_poses: &[usize]) -> String {
        let lines_source = self.get_lines_source(source, line_poses);
        format!("{self}\nAt source:\n{lines_source}")
    }
}

pub fn source_to_ir(source: &str) -> Result<Module, CompileErr> {
    let mut parser = IRParser::new(source);
    let ast = ModuleAst::parse(&mut parser)?;
    let mut module = Module::new(ArchInfo::new_host(), "");
    let mut irgen = IRGen::new(source, &ast, &module);
    irgen.generate()?;
    module.begin_gc().finish();
    Ok(module)
}

#[cfg(test)]
mod tests {
    use super::*;
    use remusys_ir::ir::{FuncClone, FuncID, IRWriteOption, IRWriter, ISubGlobalID};
    use smallvec::SmallVec;
    use std::path::PathBuf;

    fn get_example_path() -> PathBuf {
        let project_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        project_dir.join("remusys-ir-parser").join("examples")
    }

    fn load_module(filename: &str) -> Module {
        let source_path = get_example_path().join(filename);
        let source = match std::fs::read_to_string(&source_path) {
            Ok(s) => s,
            Err(e) => {
                panic!("Failed to read example source file {source_path:?}: {e}")
            }
        };
        let source_map = {
            let mut lines: SmallVec<[usize; 16]> = SmallVec::new();
            lines.push(0);
            let mut pos = 0;
            for line in source.lines() {
                pos += line.len() + 1;
                lines.push(pos);
            }
            lines
        };
        match source_to_ir(&source) {
            Ok(m) => m,
            Err(e) => {
                let e = e.dump_string(&source, &source_map);
                panic!("Failed to compile example source file {source_path:?}: {e}")
            }
        }
    }

    fn write_ir(module: &Module, name: &str) {
        let mut bytes = Vec::new();
        let mut writer = IRWriter::from_module(&mut bytes, module);
        writer.set_option(IRWriteOption::quiet());
        writer.fmt_module().unwrap();

        let output_path = get_example_path()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("target")
            .join(name);
        match std::fs::write(&output_path, &bytes) {
            Ok(_) => (),
            Err(e) => {
                panic!("Failed to write IR to file {output_path:?}: {e}")
            }
        }
    }

    #[test]
    fn test_compile() {
        let module = load_module("main.ll");
        write_ir(&module, "main_out.ll");
    }

    fn clone_func(module: &mut Module, name: &str, new_name: &str, keep_recurse: bool) {
        let func = module
            .get_global_by_name(name)
            .map(FuncID::raw_from)
            .unwrap();
        let mut fclone = FuncClone::new(module, func).unwrap();
        fclone
            .change_name(new_name)
            .keep_recurse(keep_recurse)
            .try_export()
            .unwrap();
        fclone.finish().unwrap();
    }

    /// source demo
    ///
    /// ```ignore
    /// #[repr(C, u8)]
    /// #[derive(Clone, Copy)]
    /// enum Color {
    ///     Red = 0,
    ///     Green = 1,
    ///     Blue = 2
    /// }
    ///
    /// fn color_get_name(color: Color) -> &'static str {
    ///     match color {
    ///         Color::Red => "red",
    ///         Color::Green => "green",
    ///         Color::Blue => "blue",
    ///     }
    /// }
    /// ```
    #[test]
    fn test_func_clone() {
        let mut module = load_module("clone-func.ll");

        clone_func(&mut module, "color_get_name", "color_get_name_clone", false);
        clone_func(&mut module, "fibonacci", "fibonacci_clone", false);
        clone_func(&mut module, "fibonacci", "fibonacci_rclone", true);

        write_ir(&module, "clone-func-out.ll");
    }
}
