<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{{ title }} - API Documentation</title>
    <link rel="icon" type="image/png" href="data:image/png;base64,{{ favicon_base64 }}" />
    <style>
      body {
        margin: 0;
        padding: 0;
      }
    </style>
  </head>
  <body>
    <redoc spec-url="{{ spec_url }}"></redoc>
    <!-- Fixes #826: Pin CDN version and add SRI hash -->
    <script
      src="https://cdn.redoc.ly/redoc/v2.5.2/bundles/redoc.standalone.js"
      integrity="sha384-70P5pmIdaQdVbxvjhrcTDv1uKcKqalZ3OHi7S2J+uzDl0PW8dO6L+pHOpm9EEjGJ"
      crossorigin="anonymous"
    ></script>
  </body>
</html>
