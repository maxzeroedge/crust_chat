pub const QUERY: &str = r#"
; Classes
(class_declaration
  name: (identifier) @class.name) @class.def

; Interfaces
(interface_declaration
  name: (identifier) @interface.name) @interface.def

; Methods
(method_declaration
  name: (identifier) @method.name) @method.def

; Constructors
(constructor_declaration
  name: (identifier) @method.name) @method.def

; Fields
(field_declaration
  declarator: (variable_declarator
    name: (identifier) @variable.name)) @variable.def

; Imports
(import_declaration
  (scoped_identifier) @import.path) @import.def

; Method invocations
(method_invocation
  name: (identifier) @call.name) @call.site

; Inheritance
(class_declaration
  (superclass
    (type_identifier) @inherits.name))

; Implements
(class_declaration
  (super_interfaces
    (type_list
      (type_identifier) @implements.name)))

; Enum
(enum_declaration
  name: (identifier) @enum.name) @enum.def
"#;
