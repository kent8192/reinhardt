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

- Applications must install body and decompression limits before buffering,
  deserialization, or content negotiation performs unbounded work. The parser
  APIs receive buffered bodies and do not enforce a pre-buffering limit on
  every entry point; body size, nesting, fields, multipart parts, decoded
  output, and parser work must remain bounded for every supported media type.
- Validation establishes only data shape and business rules; authorization is a
  separate server-side decision made before every read, create, update, delete,
  bulk operation, relation change, or action on the target resource and tenant.
- Protected endpoints must configure explicit writable and readable fields
  before deserialization. The default and hyperlinked serializers do not
  automatically prevent client input from mass-assigning identifiers,
  ownership, tenant, role, permission, read-only, computed, or otherwise
  protected fields, including through nested relations.
- Filters, search, lookup expressions, field selectors, and ordering use
  finite validated allowlists and bounded values. They preserve parameterized
  query construction and cannot disclose protected fields or become executable
  query structure. The `BatchValidator` uniqueness fast path currently builds
  a raw `UNION ALL` statement from table, field, and check values; callers must
  restrict that path to trusted identifiers and non-attacker-controlled values,
  or disable it until those checks use bound query values.
- Pagination and cursor state remain bound to the authorized query, tenant,
  filter scope, ordering, and API version. Collection, cursor, and version
  variants cannot enumerate objects or traverse beyond the caller's permitted
  result set. `Base64CursorEncoder` signs only its position and timestamp, and
  the database `Cursor` is unsigned encoded JSON; callers must bind cursor state
  to the authorized query scope externally because these encoders do not do so.
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
