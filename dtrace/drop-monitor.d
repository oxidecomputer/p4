::parser_accepted {
    printf("%s", copyinstr(arg0));
}

::control_table_miss {
    printf("%s", copyinstr(arg0));
}

::match_miss {
    printf("%s", copyinstr(arg0));
}

::parser_dropped {
    printf("parser dropped\n");
}

::ingress_dropped {
    printf("ingress dropped\n");
}
