//! Multi-file Titan project loading and import resolution.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use thiserror::Error;
use titan_ast::{Item, Program};
use titan_lexer::Lexer;
use titan_parser::Parser;

#[derive(Error, Debug)]
pub enum ProjectError {
    #[error("entry source not found: {0}")]
    EntryNotFound(PathBuf),
    #[error("cannot read {path}: {source}")]
    Read { path: PathBuf, #[source] source: std::io::Error },
    #[error("lexical errors in {path}: {errors}")]
    Lex { path: PathBuf, errors: String },
    #[error("parse errors in {path}: {errors}")]
    Parse { path: PathBuf, errors: String },
    #[error("cannot resolve import '{import}' from {from}")]
    ImportNotFound { import: String, from: PathBuf },
    #[error("import '{import}' resolves outside source root {root}")]
    ImportEscapesRoot { import: String, root: PathBuf },
    #[error("circular import: {chain}")]
    ImportCycle { chain: String },
    #[error("project already exists at {0}")]
    AlreadyExists(PathBuf),
    #[error("invalid project name '{0}'")]
    InvalidName(String),
    #[error("invalid manifest at {path}: {message}")]
    Manifest { path: PathBuf, message: String },
    #[error("dependency '{name}' must specify a local path in this compiler version")]
    DependencyNeedsPath { name: String },
    #[error("dependency cycle: {0}")]
    DependencyCycle(String),
    #[error("cannot create project file {path}: {source}")]
    Create { path: PathBuf, #[source] source: std::io::Error },
}

#[derive(Debug, Clone)]
pub struct SourceProject {
    pub root: PathBuf,
    pub source_root: PathBuf,
    pub entry: PathBuf,
    pub manifest: Option<crate::Manifest>,
    pub dependencies: BTreeMap<String, PathBuf>,
    pub program: Program,
    pub sources: BTreeMap<PathBuf, String>,
    pub load_order: Vec<PathBuf>,
}

impl SourceProject {
    pub fn load(entry: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let requested = entry.as_ref();
        if !requested.exists() { return Err(ProjectError::EntryNotFound(requested.to_path_buf())); }
        let entry = requested.canonicalize().map_err(|source| ProjectError::Read { path: requested.to_path_buf(), source })?;
        let root = find_project_root(&entry).unwrap_or_else(|| entry.parent().unwrap_or(Path::new(".")).to_path_buf());
        let source_root_candidate = if root.join("Titan.toml").is_file() { root.join("src") } else { root.clone() };
        let source_root = source_root_candidate.canonicalize().map_err(|source| ProjectError::Read { path: source_root_candidate, source })?;
        let manifest = if root.join("Titan.toml").is_file() {
            Some(crate::Manifest::from_dir(&root).map_err(|error| ProjectError::Manifest { path: root.join("Titan.toml"), message: error.to_string() })?)
        } else { None };
        let dependencies = if let Some(manifest) = &manifest { collect_dependencies(&root, manifest)? } else { BTreeMap::new() };
        let mut allowed_roots = vec![source_root.clone()]; allowed_roots.extend(dependencies.values().cloned());
        // Test and tool entrypoints may live outside `src`, but every resolved
        // import is still required to canonicalize inside an allowed source root.
        let mut loader = Loader { source_root: source_root.clone(), dependency_roots: dependencies.clone(), allowed_roots, sources: BTreeMap::new(), visited: HashSet::new(), stack: Vec::new(), items: Vec::new(), order: Vec::new() };
        loader.visit(&entry)?;
        Ok(Self { root, source_root, entry, manifest, dependencies, program: Program { items: loader.items }, sources: loader.sources, load_order: loader.order })
    }
}

struct Loader {
    source_root: PathBuf,
    dependency_roots: BTreeMap<String, PathBuf>,
    allowed_roots: Vec<PathBuf>,
    sources: BTreeMap<PathBuf, String>,
    visited: HashSet<PathBuf>,
    stack: Vec<PathBuf>,
    items: Vec<Item>,
    order: Vec<PathBuf>,
}

impl Loader {
    fn visit(&mut self, path: &Path) -> Result<(), ProjectError> {
        let path = path.canonicalize().map_err(|source| ProjectError::Read { path: path.to_path_buf(), source })?;
        if let Some(position) = self.stack.iter().position(|item| item == &path) {
            let mut cycle: Vec<_> = self.stack[position..].iter().map(|p| relative_display(p, &self.source_root)).collect();
            cycle.push(relative_display(&path, &self.source_root));
            return Err(ProjectError::ImportCycle { chain: cycle.join(" -> ") });
        }
        if self.visited.contains(&path) { return Ok(()); }
        let source = std::fs::read_to_string(&path).map_err(|source| ProjectError::Read { path: path.clone(), source })?;
        let mut program = parse_file(&path, &source)?;
        assign_source_files(&mut program.items, &path);
        self.sources.insert(path.clone(), source);
        self.stack.push(path.clone());
        for item in &program.items {
            if let Item::Import(import) = item {
                if import.path.first().map(String::as_str) == Some("std") { continue; }
                let imported = self.resolve_import(&import.path, &path)?;
                self.visit(&imported)?;
            }
        }
        self.stack.pop();
        self.visited.insert(path.clone());
        self.order.push(path);
        self.items.extend(program.items.into_iter().filter(|item| !matches!(item, Item::Import(_))));
        Ok(())
    }

    fn resolve_import(&self, segments: &[String], from: &Path) -> Result<PathBuf, ProjectError> {
        let display = segments.join("::");
        let (base, path_segments) = if let Some(root) = segments.first().and_then(|name| self.dependency_roots.get(name)) {
            (root, &segments[1..])
        } else {
            let owner = self.allowed_roots.iter().filter(|root| from.starts_with(root)).max_by_key(|root| root.components().count()).unwrap_or(&self.source_root);
            (owner, segments)
        };
        if path_segments.is_empty() {
            let candidate = base.join("lib.titan");
            if candidate.is_file() { return candidate.canonicalize().map_err(|source| ProjectError::Read { path: candidate, source }); }
        }
        for length in (1..=path_segments.len()).rev() {
            let relative = path_segments[..length].iter().fold(PathBuf::new(), |path, part| path.join(part));
            let candidates = [base.join(&relative).with_extension("titan"), base.join(&relative).join("mod.titan")];
            for candidate in candidates {
                if candidate.is_file() {
                    let canonical = candidate.canonicalize().map_err(|source| ProjectError::Read { path: candidate.clone(), source })?;
                    if !self.allowed_roots.iter().any(|root| canonical.starts_with(root)) { return Err(ProjectError::ImportEscapesRoot { import: display, root: base.clone() }); }
                    return Ok(canonical);
                }
            }
        }
        Err(ProjectError::ImportNotFound { import: display, from: from.to_path_buf() })
    }
}

fn collect_dependencies(root: &Path, manifest: &crate::Manifest) -> Result<BTreeMap<String, PathBuf>, ProjectError> {
    let remote_versions: BTreeMap<String,String> = if root.join("Titan.remote.lock").is_file() { crate::RemoteLockfile::read(&root.join("Titan.remote.lock")).map_err(|error|ProjectError::Manifest{path:root.join("Titan.remote.lock"),message:error.to_string()})?.packages.into_iter().map(|package|(package.name,package.version)).collect() } else { BTreeMap::new() };
    fn visit(root: &Path, project_root:&Path, remote_versions:&BTreeMap<String,String>, manifest: &crate::Manifest, output: &mut BTreeMap<String, PathBuf>, stack: &mut Vec<PathBuf>) -> Result<(), ProjectError> {
        let root = root.canonicalize().map_err(|source| ProjectError::Read { path: root.to_path_buf(), source })?;
        if let Some(position) = stack.iter().position(|path| path == &root) {
            let chain = stack[position..].iter().chain(std::iter::once(&root)).map(|path| path.display().to_string()).collect::<Vec<_>>().join(" -> ");
            return Err(ProjectError::DependencyCycle(chain));
        }
        stack.push(root.clone());
        for (alias, dependency) in &manifest.dependencies {
            let dependency_candidate = if let Some(relative)=&dependency.path { root.join(relative) } else { let version=remote_versions.get(alias).ok_or_else(||ProjectError::DependencyNeedsPath{name:alias.clone()})?; project_root.join(".titan/packages").join(alias).join(version) };
            let dependency_root = dependency_candidate.canonicalize().map_err(|source| ProjectError::Read { path: dependency_candidate, source })?;
            let dependency_manifest = crate::Manifest::from_dir(&dependency_root).map_err(|error| ProjectError::Manifest { path: dependency_root.join("Titan.toml"), message: error.to_string() })?;
            let source_candidate = dependency_root.join("src");
            let source_root = source_candidate.canonicalize().map_err(|source| ProjectError::Read { path: source_candidate, source })?;
            output.entry(alias.clone()).or_insert(source_root);
            visit(&dependency_root, project_root, remote_versions, &dependency_manifest, output, stack)?;
        }
        stack.pop(); Ok(())
    }
    let mut output = BTreeMap::new(); visit(root, root, &remote_versions, manifest, &mut output, &mut Vec::new())?; Ok(output)
}

fn assign_source_files(items: &mut [Item], path: &Path) {
    let source = path.to_string_lossy().into_owned();
    for item in items {
        match item {
            Item::Function(function) => function.source_file = Some(source.clone()),
            Item::Impl(block) => for method in &mut block.methods { method.source_file = Some(source.clone()); },
            Item::Module(module) => assign_source_files(&mut module.items, path),
            _ => {}
        }
    }
}

fn parse_file(path: &Path, source: &str) -> Result<Program, ProjectError> {
    let mut lexer = Lexer::new(source);
    let (tokens, errors) = lexer.tokenize();
    if !errors.is_empty() { return Err(ProjectError::Lex { path: path.to_path_buf(), errors: errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n") }); }
    let mut parser = Parser::new(tokens.to_vec());
    parser.parse_program().map_err(|error| {
        let mut errors = vec![error.to_string()];
        errors.extend(parser.errors().iter().skip(1).map(ToString::to_string));
        ProjectError::Parse { path: path.to_path_buf(), errors: errors.join("\n") }
    })
}

pub fn find_project_root(path: &Path) -> Option<PathBuf> {
    let absolute = path.canonicalize().ok()?;
    let start = if absolute.is_dir() { absolute.as_path() } else { absolute.parent()? };
    start.ancestors().find(|directory| directory.join("Titan.toml").is_file()).map(Path::to_path_buf)
}

pub fn default_entry(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if path.is_file() || path.extension().and_then(|value| value.to_str()) == Some("titan") { return path.to_path_buf(); }
    if let Some(root) = find_project_root(path) { return root.join("src/main.titan"); }
    path.join("main.titan")
}

pub fn create_project(path: impl AsRef<Path>, name: &str) -> Result<PathBuf, ProjectError> {
    if name.is_empty() || !name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(ProjectError::InvalidName(name.into()));
    }
    let root = path.as_ref();
    if root.exists() && root.read_dir().map(|mut entries| entries.next().is_some()).unwrap_or(true) { return Err(ProjectError::AlreadyExists(root.to_path_buf())); }
    let source_dir = root.join("src");
    std::fs::create_dir_all(&source_dir).map_err(|error| ProjectError::Create { path: root.to_path_buf(), source: error })?;
    let manifest = format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\nlicense = \"MIT\"\n\n[dependencies]\n");
    std::fs::write(root.join("Titan.toml"), manifest).map_err(|error| ProjectError::Create { path: root.join("Titan.toml"), source: error })?;
    let main_source = source_dir.join("main.titan");
    std::fs::write(&main_source, "fn main() {\n    print(\"Hello from Titan!\")\n}\n").map_err(|error| ProjectError::Create { path: main_source, source: error })?;
    Ok(root.to_path_buf())
}

fn relative_display(path: &Path, root: &Path) -> String { path.strip_prefix(root).unwrap_or(path).display().to_string() }

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary(name: &str) -> PathBuf { std::env::temp_dir().join(format!("titan-{name}-{}-{}", std::process::id(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos())) }

    #[test]
    fn loads_imports_once_in_dependency_order() {
        let root = temporary("imports"); create_project(&root, "imports").unwrap();
        std::fs::write(root.join("src/math.titan"), "fn double(x: int) -> int { x * 2 }").unwrap();
        std::fs::write(root.join("src/main.titan"), "import math\nimport math\nfn main() { double(21) }").unwrap();
        let project = SourceProject::load(root.join("src/main.titan")).unwrap();
        assert_eq!(project.load_order.len(), 2); assert_eq!(project.program.items.len(), 2);
        assert!(project.program.items.iter().filter_map(|item| if let Item::Function(function) = item { function.source_file.as_ref() } else { None }).all(|source| source.ends_with(".titan")));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn detects_import_cycles() {
        let root = temporary("cycle"); create_project(&root, "cycle").unwrap();
        std::fs::write(root.join("src/a.titan"), "import b\nfn a() { 1 }").unwrap();
        std::fs::write(root.join("src/b.titan"), "import a\nfn b() { 2 }").unwrap();
        std::fs::write(root.join("src/main.titan"), "import a\nfn main() { a() }").unwrap();
        assert!(matches!(SourceProject::load(root.join("src/main.titan")), Err(ProjectError::ImportCycle { .. })));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_local_path_dependencies() {
        let parent = temporary("dependency"); let dependency = parent.join("mathlib"); let application = parent.join("app");
        create_project(&dependency, "mathlib").unwrap();
        std::fs::rename(dependency.join("src/main.titan"), dependency.join("src/lib.titan")).unwrap();
        std::fs::write(dependency.join("src/lib.titan"), "fn triple(value: int) -> int { value * 3 }").unwrap();
        create_project(&application, "app").unwrap();
        let manifest = std::fs::read_to_string(application.join("Titan.toml")).unwrap() + "\n[dependencies.math]\npath = \"../mathlib\"\n";
        std::fs::write(application.join("Titan.toml"), manifest).unwrap();
        std::fs::write(application.join("src/main.titan"), "import math\nfn main() { triple(14) }").unwrap();
        let project = SourceProject::load(application.join("src/main.titan")).unwrap();
        assert!(project.dependencies.contains_key("math")); assert_eq!(project.program.items.len(), 2);
        std::fs::remove_dir_all(parent).unwrap();
    }
}
