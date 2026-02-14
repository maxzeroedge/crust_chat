pub const QUERY: &str = r#"
; Functions
(function_item
  name: (identifier) @function.name) @function.def

; Structs
(struct_item
  name: (type_identifier) @struct.name) @struct.def

; Enums
(enum_item
  name: (type_identifier) @enum.name) @enum.def

; Traits
(trait_item
  name: (type_identifier) @trait.name) @trait.def

; Impl methods
(impl_item
  type: (type_identifier) @impl.type
  body: (declaration_list
    (function_item
      name: (identifier) @method.name) @method.def))

; Use/imports
(use_declaration
  argument: (_) @import.path) @import.def

; Constants
(const_item
  name: (identifier) @constant.name) @constant.def

; Static variables
(static_item
  name: (identifier) @static.name) @static.def

; Function calls
(call_expression
  function: (identifier) @call.name) @call.site

; Method calls
(call_expression
  function: (field_expression
    field: (field_identifier) @call.method_name)) @call.method_site
"#;
