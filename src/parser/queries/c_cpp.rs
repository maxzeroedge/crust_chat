pub const C_QUERY: &str = r#"
; Functions
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @function.name)) @function.def

; Struct definitions
(struct_specifier
  name: (type_identifier) @struct.name) @struct.def

; Enum definitions
(enum_specifier
  name: (type_identifier) @enum.name) @enum.def

; Variable declarations
(declaration
  declarator: (init_declarator
    declarator: (identifier) @variable.name)) @variable.def

; Includes
(preproc_include
  path: (_) @import.path) @import.def

; Function calls
(call_expression
  function: (identifier) @call.name) @call.site
"#;

pub const CPP_QUERY: &str = r#"
; Functions
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @function.name)) @function.def

; Classes
(class_specifier
  name: (type_identifier) @class.name) @class.def

; Struct definitions
(struct_specifier
  name: (type_identifier) @struct.name) @struct.def

; Enum definitions
(enum_specifier
  name: (type_identifier) @enum.name) @enum.def

; Variable declarations
(declaration
  declarator: (init_declarator
    declarator: (identifier) @variable.name)) @variable.def

; Includes
(preproc_include
  path: (_) @import.path) @import.def

; Namespaces
(namespace_definition
  name: (identifier) @module.name) @module.def

; Function calls
(call_expression
  function: (identifier) @call.name) @call.site

; Inheritance
(base_class_clause
  (type_identifier) @inherits.name)
"#;
