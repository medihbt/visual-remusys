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
    use std::{io::Write, path::PathBuf};

    fn get_example_path() -> PathBuf {
        let project_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        project_dir.join("remusys-ir-parser").join("examples")
    }

    #[test]
    fn test_compile() {
        let source_path = get_example_path().join("main.ll");
        let source = match std::fs::read_to_string(&source_path) {
            Ok(s) => s,
            Err(e) => {
                panic!("Failed to read example source file {source_path:?}: {e}")
            }
        };

        let lines_map = {
            let mut lines: SmallVec<[usize; 16]> = SmallVec::new();
            lines.push(0);
            let mut pos = 0;
            for line in source.lines() {
                pos += line.len() + 1;
                lines.push(pos);
            }
            lines
        };
        let module = match source_to_ir(&source) {
            Ok(m) => m,
            Err(e) => {
                panic!("{}", e.dump_string(&source, &lines_map))
            }
        };

        let mut bytes = Vec::new();
        let mut writer = IRWriter::from_module(&mut bytes, &module);
        writer.set_option(IRWriteOption::quiet());
        writer.fmt_module().unwrap();

        let mut stdout = std::io::stdout().lock();
        stdout.write_all(&bytes).unwrap();
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
        let source = r#"
        @str_red    = internal constant [3 x i8] c"red", align 8
        @str_green  = internal constant [5 x i8] c"green", align 8
        @str_blue   = internal constant [4 x i8] c"blue", align 8

        define dso_local {ptr, i64} @color_get_name(i8 %color) {
        entry:
            switch i8 %color, label %default [
                i8 0, label %case_red
                i8 1, label %case_green
                i8 2, label %case_blue
            ]
        case_red:
            ; ret {ptr, i64} {ptr @str_red, i64 3}
            br label %finish
        case_green:
            ; ret {ptr, i64} {ptr @str_green, i64 5}
            br label %finish
        case_blue:
            ; ret {ptr, i64} {ptr @str_blue, i64 4}
            br label %finish
        default:
            unreachable
        finish:
            %retval = phi {ptr, i64}
                [{ptr @str_red, i64 3}, %case_red],
                [{ptr @str_green, i64 5}, %case_green],
                [{ptr @str_blue, i64 4}, %case_blue]
            ret {ptr, i64} %retval
        }

        @str_true  = internal constant [4 x i8] c"true", align 8
        @str_false = internal constant [5 x i8] c"false", align 8

        define dso_local {ptr, i64} @boolean_to_string(i1 %b) {
            %res = select i1 %b, {ptr, i64} {ptr @str_true, i64 4}, {ptr @str_false, i64 5}
            ret {ptr, i64} %res
        }
        "#;

        let lines_map = {
            let mut lines: SmallVec<[usize; 16]> = SmallVec::new();
            lines.push(0);
            let mut pos = 0;
            for line in source.lines() {
                pos += line.len() + 1;
                lines.push(pos);
            }
            lines
        };
        let mut module = match source_to_ir(source) {
            Ok(m) => m,
            Err(e) => {
                panic!("{}", e.dump_string(source, &lines_map))
            }
        };

        let func = module
            .get_global_by_name("color_get_name")
            .map(FuncID::raw_from)
            .unwrap();
        let mut fclone = FuncClone::new(&mut module, func).unwrap();
        fclone.change_name("cloned_func").try_export().unwrap();
        fclone.finish().unwrap();

        let mut bytes = Vec::new();
        let mut writer = IRWriter::from_module(&mut bytes, &module);
        writer.set_option(IRWriteOption::quiet());
        writer.fmt_module().unwrap();

        let mut stdout = std::io::stdout().lock();
        stdout.write_all(&bytes).unwrap();
    }
}
