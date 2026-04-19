# Import blocks for recovering resources into Terraform state.
# Currently empty: all resources are freshly created in the CI sub-account (495680546359).
#
# Previous import blocks referenced management-account resources and were removed
# during the migration from management account (819171434490) to CI sub-account.
#
# If you need to import an existing resource, add an import block here:
# import {
#   to = <resource_type>.<resource_name>
#   id = "<resource_id>"
# }
