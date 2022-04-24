// IPv4 header without options
header IPv4_no_options_h {
   bit<4>   version;
   bit<4>   ihl;
   bit<8>   diffserv;
   bit<16>  totalLen;
   bit<16>  identification;
   bit<3>   flags;
   bit<13>  fragOffset;
   bit<8>   ttl;
   bit<8>   protocol;
   bit<16>  hdrChecksum;
   bit<32>  srcAddr;
   bit<32>  dstAddr;
}
header IPv4_options_h {
   varbit<320> options;
}

struct Parsed_headers {
    // Some fields omitted
    IPv4_no_options_h ipv4;
    IPv4_options_h    ipv4options;
}

// TODO parse error decls
//error { InvalidIPv4Header }

parser Top(packet_in b, out Parsed_headers headers) {
   // Some states omitted

   state parse_ipv4 {
       b.extract(headers.ipv4);
       verify(headers.ipv4.ihl >= 5, error.InvalidIPv4Header);
       transition select (headers.ipv4.ihl) {
           5: dispatch_on_protocol;
           _: parse_ipv4_options;
       }
   }

   state parse_ipv4_options {
       // use information in the ipv4 header to compute the number
       // of bits to extract
       b.extract(headers.ipv4options); //,
                 // TODO (bit<32>)(((bit<16>)headers.ipv4.ihl - 5) * 32));
       transition dispatch_on_protocol;
   }
}
