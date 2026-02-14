pub const QUERY: &str = r#"
; Functions
(function_declaration
  name: (identifier) @function.name) @function.def

; Methods
(method_declaration
  name: (field_identifier) @method.name) @method.def

; Structs
(type_declaration
  (type_spec
    name: (type_identifier) @struct.name
    type: (struct_type))) @struct.def

; Interfaces
(type_declaration
  (type_spec
    name: (type_identifier) @interface.name
    type: (interface_type))) @interface.def

; Imports
(import_spec
  path: (interpreted_string_literal) @import.path) @import.def

; Variable declarations
(short_var_declaration
  left: (expression_list
    (identifier) @variable.name)) @variable.def

(var_declaration
  (var_spec
    name: (identifier) @variable.name)) @variable.def

; Constants
(const_declaration
  (const_spec
    name: (identifier) @constant.name)) @constant.def

; Function calls
(call_expression
  function: (identifier) @call.name) @call.site

(call_expression
  function: (selector_expression
    field: (field_identifier) @call.method_name)) @call.method_site
"#;
