use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::files::{FileId, FileTable};
use super::lexer::{self, make_token, Token, TokenKind};
use crate::error::{AssemblerError, Result};

#[derive(Debug)]
pub struct Preprocessed {
    pub files: FileTable,
    pub tokens: Vec<Token>,
}

#[derive(Clone)]
struct MacroDefinition {
    replacement: Option<TokenKind>,
}

struct ConditionalFrame {
    loc_file: FileId,
    loc_span: std::ops::Range<usize>,
    parent_active: bool,
    condition_met: bool,
    seen_else: bool,
}

pub struct Preprocessor {
    files: FileTable,
    defines: HashMap<String, MacroDefinition>,
    include_stack: Vec<PathBuf>,
}

impl Default for Preprocessor {
    fn default() -> Self {
        Self {
            files: FileTable::new(),
            defines: HashMap::new(),
            include_stack: Vec::new(),
        }
    }
}

enum Directive {
    Include(String),
    Define(String, Option<String>),
    Undef(String),
    IfDef(String),
    IfNDef(String),
    Else,
    EndIf,
}

impl Preprocessor {
    pub fn preprocess_source(source: &str) -> Result<Preprocessed> {
        let mut preprocessor = Self::default();
        let root_file = preprocessor
            .files
            .add(PathBuf::from("<source>"), source.to_string());
        let mut tokens = preprocessor
            .process_loaded_file(root_file)
            .map_err(|error| error.with_files(preprocessor.files.clone()))?;
        preprocessor.finish(&mut tokens, root_file);
        Ok(Preprocessed {
            files: preprocessor.files,
            tokens,
        })
    }

    pub fn preprocess_file(path: &Path) -> Result<Preprocessed> {
        let mut preprocessor = Self::default();
        let root_path = std::fs::canonicalize(path).map_err(|error| {
            AssemblerError::IoError(format!("Failed to resolve file {}: {}", path.display(), error))
        })?;
        let root_file = preprocessor.load_file(&root_path)?;
        let mut tokens = preprocessor
            .process_loaded_file(root_file)
            .map_err(|error| error.with_files(preprocessor.files.clone()))?;
        preprocessor.finish(&mut tokens, root_file);
        Ok(Preprocessed {
            files: preprocessor.files,
            tokens,
        })
    }

    fn finish(&self, tokens: &mut Vec<Token>, root_file: FileId) {
        let source = self.files.source(root_file);
        tokens.push(make_token(
            TokenKind::Eof,
            root_file,
            source,
            source.len()..source.len(),
        ));
    }

    fn load_file(&mut self, path: &Path) -> Result<FileId> {
        let source = std::fs::read_to_string(path).map_err(|error| {
            AssemblerError::IoError(format!("Failed to read file {}: {}", path.display(), error))
        })?;

        let file = self.files.add(path.to_path_buf(), source);
        Ok(file)
    }

    fn process_loaded_file(&mut self, file: FileId) -> Result<Vec<Token>> {
        let path = self.files.path(file).clone();
        self.include_stack.push(path.clone());

        let source = self.files.source(file).to_string();
        let mut tokens = Vec::new();
        let mut frames = Vec::new();
        let mut offset = 0usize;

        for line in source.split_inclusive('\n') {
            let span = offset..(offset + line.len());
            self.process_line(file, &path, &source, line, span.clone(), &mut frames, &mut tokens)?;
            offset += line.len();
        }

        if !source.ends_with('\n') && !tokens.is_empty() {
            tokens.push(make_token(
                TokenKind::Newline,
                file,
                &source,
                source.len()..source.len(),
            ));
        }

        self.include_stack.pop();

        if let Some(frame) = frames.pop() {
            return Err(AssemblerError::parse(
                frame.loc_file,
                frame.loc_span,
                "Unclosed conditional directive",
            ));
        }

        Ok(tokens)
    }

    fn process_line(
        &mut self,
        file: FileId,
        path: &Path,
        source: &str,
        line: &str,
        span: std::ops::Range<usize>,
        frames: &mut Vec<ConditionalFrame>,
        tokens: &mut Vec<Token>,
    ) -> Result<()> {
        let Some(directive) = self.parse_directive(file, line, span.clone())? else {
            if self.is_active(frames) {
                tokens.extend(self.expand_line(file, source, line, span.start)?);
            }
            return Ok(());
        };

        self.apply_directive(file, path, source, directive, span, frames, tokens)
    }

    fn parse_directive(
        &self,
        file: FileId,
        line: &str,
        span: std::ops::Range<usize>,
    ) -> Result<Option<Directive>> {
        let trimmed = line.trim_start();
        let Some(body) = trimmed.strip_prefix('#') else {
            return Ok(None);
        };

        let body = body.trim();
        if body.is_empty() {
            return Ok(None);
        }

        let (name, rest) = split_once_whitespace(body);
        let directive = match name {
            "include" => Directive::Include(parse_required_value(file, span, rest, "#include")?),
            "define" => {
                let (macro_name, value) = parse_name_and_value(file, span, rest, "#define")?;
                Directive::Define(macro_name, value)
            }
            "undef" => Directive::Undef(parse_required_name(file, span, rest, "#undef")?),
            "ifdef" => Directive::IfDef(parse_required_name(file, span, rest, "#ifdef")?),
            "ifndef" => Directive::IfNDef(parse_required_name(file, span, rest, "#ifndef")?),
            "else" => Directive::Else,
            "endif" => Directive::EndIf,
            _ => return Ok(None),
        };

        Ok(Some(directive))
    }

    fn apply_directive(
        &mut self,
        file: FileId,
        path: &Path,
        source: &str,
        directive: Directive,
        span: std::ops::Range<usize>,
        frames: &mut Vec<ConditionalFrame>,
        tokens: &mut Vec<Token>,
    ) -> Result<()> {
        match directive {
            Directive::IfDef(name) => {
                let parent_active = self.is_active(frames);
                let condition_met = self.defines.contains_key(&name);
                frames.push(ConditionalFrame {
                    loc_file: file,
                    loc_span: span.clone(),
                    parent_active,
                    condition_met,
                    seen_else: false,
                });
            }
            Directive::IfNDef(name) => {
                let parent_active = self.is_active(frames);
                let condition_met = !self.defines.contains_key(&name);
                frames.push(ConditionalFrame {
                    loc_file: file,
                    loc_span: span.clone(),
                    parent_active,
                    condition_met,
                    seen_else: false,
                });
            }
            Directive::Else => {
                let Some(frame) = frames.last_mut() else {
                    return Err(AssemblerError::parse(file, span, "Unexpected #else"));
                };
                if frame.seen_else {
                    return Err(AssemblerError::parse(file, span, "Duplicate #else"));
                }
                frame.seen_else = true;
                frame.condition_met = !frame.condition_met;
            }
            Directive::EndIf => {
                if frames.pop().is_none() {
                    return Err(AssemblerError::parse(file, span, "Unexpected #endif"));
                }
            }
            Directive::Include(_) | Directive::Define(_, _) | Directive::Undef(_) if !self.is_active(frames) => {}
            Directive::Include(raw_path) => {
                let include_path = resolve_include(path, &raw_path, file, span.clone())?;
                let include_file = self.load_file_from_include(&include_path, file, span.clone())?;
                tokens.extend(self.process_loaded_file(include_file)?);
            }
            Directive::Define(name, value) => {
                let replacement = match value {
                    Some(value) => Some(self.parse_macro_replacement(file, span.clone(), &value)?),
                    None => None,
                };
                self.defines.insert(name, MacroDefinition { replacement });
            }
            Directive::Undef(name) => {
                self.defines.remove(&name);
            }
        }

        if self.is_active(frames) && tokens.last().map(|token| !matches!(token.kind, TokenKind::Newline)).unwrap_or(true) {
            tokens.push(make_token(TokenKind::Newline, file, source, span.end..span.end));
        }

        Ok(())
    }

    fn expand_line(
        &self,
        file: FileId,
        source: &str,
        line: &str,
        line_offset: usize,
    ) -> Result<Vec<Token>> {
        let line_tokens = lexer::tokenize(line, file).map_err(|error| error.with_span_offset(line_offset))?;
        let mut expanded = Vec::new();

        for token in line_tokens {
            if matches!(token.kind, TokenKind::Eof) {
                continue;
            }

            let span = (token.span.start + line_offset)..(token.span.end + line_offset);
            let kind = match &token.kind {
                TokenKind::Ident(name) => self
                    .defines
                    .get(name)
                    .and_then(|definition| definition.replacement.clone())
                    .unwrap_or_else(|| token.kind.clone()),
                _ => token.kind.clone(),
            };

            expanded.push(make_token(kind, file, source, span));
        }

        Ok(expanded)
    }

    fn parse_macro_replacement(
        &self,
        file: FileId,
        span: std::ops::Range<usize>,
        value: &str,
    ) -> Result<TokenKind> {
        let tokens = lexer::tokenize(value, file)?;
        let filtered: Vec<_> = tokens
            .into_iter()
            .filter(|token| !matches!(token.kind, TokenKind::Eof | TokenKind::Newline))
            .collect();

        if filtered.len() != 1 {
            return Err(AssemblerError::parse(
                file,
                span,
                "#define values must expand to exactly one token",
            ));
        }

        Ok(filtered.into_iter().next().expect("single token").kind)
    }

    fn is_active(&self, frames: &[ConditionalFrame]) -> bool {
        frames.iter().all(|frame| frame.parent_active && frame.condition_met)
    }

    fn load_file_from_include(
        &mut self,
        path: &Path,
        file: FileId,
        span: std::ops::Range<usize>,
    ) -> Result<FileId> {
        if self.include_stack.iter().any(|active| active == path) {
            return Err(AssemblerError::parse(
                file,
                span,
                format!("Circular include detected for {}", path.display()),
            ));
        }

        self.load_file(path).map_err(|error| match error {
            AssemblerError::IoError(message) => AssemblerError::parse(file, span, message),
            other => other,
        })
    }
}

fn resolve_include(
    current_path: &Path,
    raw_path: &str,
    file: FileId,
    span: std::ops::Range<usize>,
) -> Result<PathBuf> {
    let include_path = parse_include_path(raw_path, file, span.clone())?;
    let base_dir = current_path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::canonicalize(base_dir.join(include_path)).map_err(|error| {
        AssemblerError::parse(
            file,
            span,
            format!("Failed to resolve include {}: {}", raw_path.trim(), error),
        )
    })
}

fn parse_include_path(value: &str, file: FileId, span: std::ops::Range<usize>) -> Result<&str> {
    value
        .trim()
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    .ok_or_else(|| AssemblerError::parse(file, span, "#include expects a quoted path"))
}

fn parse_required_name(
    file: FileId,
    span: std::ops::Range<usize>,
    rest: &str,
    directive: &str,
) -> Result<String> {
    let name = rest.trim();
    if !is_identifier(name) {
        return Err(AssemblerError::parse(
            file,
            span,
            format!("{directive} expects an identifier"),
        ));
    }
    Ok(name.to_string())
}

fn parse_required_value(
    file: FileId,
    span: std::ops::Range<usize>,
    rest: &str,
    directive: &str,
) -> Result<String> {
    let value = rest.trim();
    if value.is_empty() {
        return Err(AssemblerError::parse(
            file,
            span,
            format!("{directive} expects a value"),
        ));
    }
    Ok(value.to_string())
}

fn parse_name_and_value(
    file: FileId,
    span: std::ops::Range<usize>,
    rest: &str,
    directive: &str,
) -> Result<(String, Option<String>)> {
    let trimmed = rest.trim();
    if trimmed.is_empty() {
        return Err(AssemblerError::parse(
            file,
            span,
            format!("{directive} expects an identifier"),
        ));
    }

    let (name, value) = split_once_whitespace(trimmed);
    if !is_identifier(name) {
        return Err(AssemblerError::parse(
            file,
            span,
            format!("{directive} expects an identifier"),
        ));
    }

    let value = if value.trim().is_empty() {
        None
    } else {
        Some(value.trim().to_string())
    };

    Ok((name.to_string(), value))
}

fn split_once_whitespace(input: &str) -> (&str, &str) {
    let split_at = input.find(char::is_whitespace).unwrap_or(input.len());
    (&input[..split_at], &input[split_at..])
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }

    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}