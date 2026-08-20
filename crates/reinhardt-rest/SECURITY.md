# reinhardt-rest Security Policy

## System and Scope

This policy inherits the repository [Security Policy](../../SECURITY.md) and
the framework-crate [Security Policy](../SECURITY.md). Policies compose from
the repository root to this crate; this closest policy wins on conflict.

`reinhardt-rest` provides REST parsers, serializers, authentication adapters,
filters, search, ordering, pagination, versioning, throttling, browsable API
rendering, and schema generation. Bodies, media types, parser selections,
serialized fields, query parameters, cursor tokens, versions, credentials, and
schema metadata are attacker-controlled until their corresponding controls
validate them.

## Security Invariants

- Parser and decompression limits apply before buffering, deserialization, or
  content negotiation performs unbounded work. Body size, nesting, fields,
  multipart parts, decoded output, and parser work remain bounded for every
  supported media type.
- Validation establishes only data shape and business rules; authorization is a
  separate server-side decision made before every read, create, update, delete,
  bulk operation, relation change, or action on the target resource and tenant.
- Serializers use explicit writable and readable fields. Client input cannot
  mass-assign identifiers, ownership, tenant, role, permission, read-only,
  computed, or otherwise protected fields, including through nested relations
  or alternate serializer forms.
- Filters, search, lookup expressions, field selectors, and ordering use
  finite validated allowlists and bounded values. They preserve parameterized
  query construction and cannot disclose protected fields or become executable
  query structure.
- Pagination and cursor state remain bound to the authorized query, tenant,
  filter scope, ordering, and API version. Collection, cursor, and version
  variants cannot enumerate objects or traverse beyond the caller's permitted
  result set.
- API versions, parser choices, and content-negotiation variants enforce the
  same authentication, authorization, validation, isolation, and error
  protections for an equivalent operation; an alternate representation cannot
  be a weaker endpoint.
- Throttling identifies callers through authenticated principal or validated
  network identity and cannot be partitioned, bypassed, or targeted through
  client-supplied identity headers, credentials, or route metadata.
- Browsable API pages and form values use context-appropriate escaping and do
  not render untrusted request, response, schema, or error data as executable
  HTML, attributes, URLs, or scripts.
- Generated schemas, OpenAPI documents, and interactive documentation exclude
  credentials, tokens, private endpoints, internal-only fields, authorization
  internals, and other secrets. Documentation exposure does not grant access
  beyond the corresponding API operation.

## Reportable Findings

Report pre-parser resource exhaustion, validation-as-authorization, mass
assignment, unsafe filter/search/ordering structure, pagination or versioning
authorization bypass, spoofable throttling, browsable-output injection,
secret-bearing schemas, or weaker parser and negotiation variants.
