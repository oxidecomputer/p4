# Endianness

The basic rules for endiannes follow. Generally speaking numeric fields are in
big endian when they come in off the wire, little endian while in the program,
and transformed back to big endian on the way back out onto the wire. We refer
to this as confused endian.

1. All numeric packet field data is big endian when enters and leaves a p4
   program.
2. All numeric data, including packet fields is little endian inside a p4
   program.
3. Table keys with the `exact` and `range` type defined over bit types are in
   little endian.
4. Table keys with the `lpm` type are in the byte order they appear on the wire.
