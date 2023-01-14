::parser_transition{
    printf("%s", copyinstr(arg0));
}

::parser_dropped {
    printf("parser dropped\n");
}

::parser_accepted {
    printf("%s", copyinstr(arg0));
}

::control_apply{
    printf("%s", copyinstr(arg0));
}

::ingress_dropped {
    printf("ingress dropped\n");
}

/*::egress_dropped {
    printf("egress dropped\n");
}*/

::ingress_accepted {
    printf("ingress accepted\n");
}

/*::egress_accepted {
    printf("egress accepted\n");
}*/

::control_table_hit {
    printf("%s", copyinstr(arg0));
}

::control_table_miss {
    printf("%s", copyinstr(arg0));
}

::match_miss {
    printf("%s", copyinstr(arg0));
}
