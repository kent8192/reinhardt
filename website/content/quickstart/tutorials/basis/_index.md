+++
title = "Basics Tutorials"
description = "Core concepts and fundamentals of Reinhardt."
sort_by = "weight"
weight = 10

[extra]
sidebar_weight = 10
+++


# Reinhardt Basis Tutorial

Learn the fundamentals of the Reinhardt framework by building a real-world polling application.

## Overview

This tutorial series will guide you through creating a fully functional polling application from scratch. You'll learn Reinhardt's core concepts including models, views, templates, forms, testing, and the admin interface.

## Who This Tutorial Is For

This tutorial is designed for:

- Developers new to Reinhardt who want to learn the framework from the ground up
- Django developers transitioning to Rust and Reinhardt
- Anyone building web applications with server-rendered templates

## Prerequisites

Before starting this tutorial, you should have:

- Basic knowledge of Rust programming
- Familiarity with Cargo (Rust's package manager)
- Understanding of HTTP concepts and web development
- A code editor or IDE

## What You'll Build

Throughout this tutorial, you'll build a polling application where users can:

- View polls and their choices
- Vote on polls
- See voting results

As an administrator, you'll be able to:

- Create and manage polls through the admin interface
- Add and edit choices for each poll
- Customize the admin interface to your needs

## Tutorial Structure

This tutorial is divided into seven parts:

### [Part 1: Project Setup](1-project-setup/)

- Install Reinhardt
- Create a new project
- Run the development server
- Create your first view and URL configuration

### [Part 2: Models and Database](2-models-and-database/)

- Set up the database
- Create models (Question and Choice)
- Use the database API
- Introduction to the Reinhardt admin

### [Part 3: Views and URLs](3-views-and-urls/)

- Write more views
- Connect views to URLs
- Use templates to render HTML
- Implement shortcut functions

### [Part 4: Forms and Generic Views](4-forms-and-generic-views/)

- Create HTML forms
- Process form submissions
- Use generic views to reduce code
- Implement the voting functionality

### [Part 5: Testing](5-testing/)

- Write automated tests
- Test models and views
- Use the test client
- Follow testing best practices

### [Part 6: Static Files](6-static-files/)

- Add stylesheets (CSS)
- Include images
- Organize static files
- Configure static file serving

### [Part 7: Admin Customization](7-admin-customization/)

- Customize the admin form
- Add related objects
- Customize the change list
- Modify admin templates

## Recommended Learning Path

Work through the tutorials in order, as each part builds on the previous ones. You can skip ahead if you're already familiar with certain concepts, but we recommend following along sequentially for the best learning experience.

## Getting Help

If you encounter issues while following this tutorial:

- Check the [Reinhardt documentation](https://github.com/kent8192/reinhardt-web)
- Review the [Getting Started Guide](/quickstart/getting-started/)
- Look at the [Feature Flags Guide](/docs/feature-flags/)
- Examine the individual crate READMEs for detailed information

## Comparison with REST Tutorial

If you're also interested in building REST APIs, check out the [REST Tutorial](../rest/quickstart/). The key differences are:

- **Basis Tutorial**: Focuses on full-stack web applications with server-rendered templates
- **REST Tutorial**: Focuses on building APIs with JSON responses and serializers

Both tutorials teach fundamental Reinhardt concepts, so learning one will help you understand the other.

## Let's Get Started!

Ready to begin? Head over to [Part 1: Project Setup](1-project-setup/) to create your first Reinhardt project!