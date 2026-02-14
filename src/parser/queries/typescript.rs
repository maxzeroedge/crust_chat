pub const QUERY: &str = r#"
; Functions
(function_declaration
  name: (identifier) @function.name) @function.def

; Arrow functions
(lexical_declaration
  (variable_declarator
    name: (identifier) @function.name
    value: (arrow_function) @function.def))

; Classes
(class_declaration
  name: (type_identifier) @class.name) @class.def

; Methods
(method_definition
  name: (property_identifier) @method.name) @method.def

; Interfaces
(interface_declaration
  name: (type_identifier) @interface.name) @interface.def

; Type aliases
(type_alias_declaration
  name: (type_identifier) @type_alias.name) @type_alias.def

; Enums
(enum_declaration
  name: (identifier) @enum.name) @enum.def

; Imports
(import_statement
  source: (string) @import.path) @import.def

; Variable declarations
(lexical_declaration
  (variable_declarator
    name: (identifier) @variable.name)) @variable.def

; Function calls
(call_expression
  function: (identifier) @call.name) @call.site

(call_expression
  function: (member_expression
    property: (property_identifier) @call.method_name)) @call.method_site

; Class heritage
(class_heritage
  (identifier) @inherits.name)

; Implements
(class_heritage
  (type_identifier) @implements.name)
"#;
