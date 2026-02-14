pub const QUERY: &str = r#"
; Functions
(function_declaration
  name: (identifier) @function.name) @function.def

; Arrow functions assigned to variables
(lexical_declaration
  (variable_declarator
    name: (identifier) @function.name
    value: (arrow_function) @function.def))

; Classes
(class_declaration
  name: (identifier) @class.name) @class.def

; Methods
(method_definition
  name: (property_identifier) @method.name) @method.def

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

; Class inheritance
(class_heritage
  (identifier) @inherits.name)
"#;
