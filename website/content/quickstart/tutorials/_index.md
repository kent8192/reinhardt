+++
title = "Tutorials"
description = "Step-by-step tutorials for building with Reinhardt."
sort_by = "weight"
weight = 20

[extra]
sidebar_weight = 20
+++

# Tutorials

Welcome to the Reinhardt tutorials! Choose a learning path below.

<div class="tutorial-cards">

<div class="tutorial-card" markdown="1">

## [Basics Tutorials →](basis/)

Build a full-stack polling application with WASM-based frontend and server-side rendering.

**Topics:** Project setup, Models, Server Functions, Forms, Testing, Static Files, Admin

**Best for:** Developers building monolithic web apps with integrated Rust frontend.

</div>

<div class="tutorial-card" markdown="1">

## [REST API Tutorials →](rest/)

Create RESTful APIs for mobile apps, SPAs, and external clients.

**Topics:** HTTP Decorators, Serialization, Authentication, Permissions, ViewSets

**Best for:** Developers building backend APIs for multiple clients.

</div>

</div>

## Which Tutorial Should I Choose?

| Aspect | Basics Tutorial | REST API Tutorial |
|--------|-----------------|-------------------|
| **Architecture** | WASM + SSR (full-stack) | REST API (backend only) |
| **Frontend** | Built-in (Rust/WASM) | External (React, Vue, mobile) |
| **Communication** | Server Functions (RPC) | HTTP JSON API |
| **Use Case** | Monolithic web apps | APIs for multiple clients |

## Prerequisites

Before starting, you should have:

- Basic knowledge of Rust programming
- Familiarity with Cargo (Rust's package manager)
- Understanding of HTTP concepts

## Getting Help

- Check the [Getting Started Guide](../getting-started/)
- Review the [Feature Flags Guide](../../docs/feature-flags/)
- Visit the [GitHub Repository](https://github.com/kent8192/reinhardt-web)