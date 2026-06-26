# ADR-0002: Use React as Frontend Framework

**Status:** Accepted

**Date:** 2026-06-26

## Context

We need a frontend framework for the Tauri desktop app. The framework must:

- Integrate well with Tauri's webview-based rendering
- Have a mature ecosystem of UI libraries (component libraries, editors, markdown renderers)
- Support modern reactive patterns (hooks, streaming for LLM responses)
- Be performant for a code-editing UI with large document rendering
- Have good TypeScript support

## Decision

We will use **React 19** as the frontend framework.

React was chosen over alternatives (Svelte 5, Vue 3, SolidJS) because:

- **Ecosystem dominance**: Every critical UI dependency we need has a first-class React wrapper: Monaco (`@monaco-editor/react`), react-markdown, TanStack Query, xterm.js (`@xterm/xterm`), shadcn/ui
- **Streaming support**: React 19's `use()` hook and Suspense integrate naturally with streaming LLM responses
- **TypeScript**: Best-in-class type inference with generics, discriminated unions, and template literals
- **Hiring/community**: Largest pool of developers, documentation, and StackOverflow answers
- **Tooling**: Vite (fast HMR), Vitest (testing), Storybook (component dev) all have first-class React support

## Consequences

### Positive

- Every major UI component we evaluated has a mature React binding — no adapter work needed
- React 19's concurrent features can keep the UI responsive during streaming LLM output
- Large ecosystem for state management (Zustand), async data (TanStack Query), and routing
- Tailwind CSS + shadcn/ui combo proven in thousands of production apps

### Negative

- Larger bundle size vs Svelte (~12KB gzipped runtime vs 0KB)
- React's re-render model requires care (memoization, virtualization) for large codebase views
- JSX is more verbose than Svelte templates

## Alternatives Considered

- **Svelte 5**: Rejected because several key dependencies lack mature Svelte bindings, requiring custom wrapper work
- **Vue 3**: Rejected despite good ecosystem — smaller community for the specific tooling we need
- **SolidJS**: Rejected due to smaller ecosystem and less mature SSR/streaming patterns
