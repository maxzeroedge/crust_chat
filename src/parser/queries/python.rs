pub const QUERY: &str = r#"
; Functions
(function_definition
  name: (identifier) @function.name) @function.def

; Classes
(class_definition
  name: (identifier) @class.name) @class.def

; Methods (inside class)
(class_definition
  body: (block
    (function_definition
      name: (identifier) @method.name) @method.def))

; Imports
(import_statement
  name: (dotted_name) @import.path) @import.def

(import_from_statement
  module_name: (dotted_name) @import.module) @import.from_def

; Assignments (top-level variables)
(assignment
  left: (identifier) @variable.name) @variable.def

; Function calls
(call
  function: (identifier) @call.name) @call.site

(call
  function: (attribute
    attribute: (identifier) @call.method_name)) @call.method_site

; Class inheritance
(class_definition
  superclasses: (argument_list
    (identifier) @inherits.name))
"#;
