<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{{ title }} - Swagger UI</title>
    <link rel="icon" type="image/png" href="data:image/png;base64,{{ favicon_base64 }}" />
    <!-- Fixes #826: Pin CDN version and add SRI hash -->
    <link
      rel="stylesheet"
      type="text/css"
      href="https://unpkg.com/swagger-ui-dist@5.31.1/swagger-ui.css"
      integrity="sha384-KX9Rx9vM1AmUNAn07bPAiZhFD4C8jdNgG6f5MRNvR+EfAxs2PmMFtUUazui7ryZQ"
      crossorigin="anonymous"
    />
    <style>
      html {
        box-sizing: border-box;
        overflow: -moz-scrollbars-vertical;
        overflow-y: scroll;
      }
      *,
      *:before,
      *:after {
        box-sizing: inherit;
      }
      body {
        margin: 0;
        padding: 0;
      }
    </style>
  </head>
  <body>
    <div id="swagger-ui"></div>
    <!-- Fixes #826: Pin CDN versions and add SRI hashes -->
    <script
      src="https://unpkg.com/swagger-ui-dist@5.31.1/swagger-ui-bundle.js"
      integrity="sha384-o9idN8HE6/V6SAewgnr6/5nz7+Npt5J0Cb4tNyXK8pycsVmgl1ZNbRS7tlEGxd+J"
      crossorigin="anonymous"
    ></script>
    <script
      src="https://unpkg.com/swagger-ui-dist@5.31.1/swagger-ui-standalone-preset.js"
      integrity="sha384-FjFI+0PRyd5aAEoduVmUYk65iDC5+onNzam4WJtbAKKBim/kWvGUMk/2+9qRaYZb"
      crossorigin="anonymous"
    ></script>
    <script>
      window.onload = function () {
        window.ui = SwaggerUIBundle({
          url: "{{ spec_url }}",
          dom_id: "#swagger-ui",
          deepLinking: true,
          presets: [SwaggerUIBundle.presets.apis, SwaggerUIStandalonePreset],
          plugins: [SwaggerUIBundle.plugins.DownloadUrl],
          layout: "StandaloneLayout",
        });
      };
    </script>
  </body>
</html>
