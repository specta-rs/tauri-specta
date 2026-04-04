use crate::Commands;

/// A tanstack query type alias for [`Commands`].
pub type Queries<R> = Commands<R>;

/// A tanstack mutation type alias for [`Commands`].
pub type Mutations<R> = Commands<R>;

/// Which TanStack Query framework to target for imports.
#[derive(Debug, Clone, Copy)]
pub enum TanstackFramework {
    /// `@tanstack/react-query`
    React,
    /// `@tanstack/solid-query`
    Solid,
    /// `@tanstack/vue-query`
    Vue,
    /// `@tanstack/svelte-query`
    Svelte,
}

impl TanstackFramework {
    /// Returns the npm package name for this framework.
    pub fn package_name(&self) -> &'static str {
        match self {
            Self::React => "@tanstack/react-query",
            Self::Solid => "@tanstack/solid-query",
            Self::Vue => "@tanstack/vue-query",
            Self::Svelte => "@tanstack/svelte-query",
        }
    }
}
