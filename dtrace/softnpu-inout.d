::parser_accepted {
    printf("%s", copyinstr(arg0));
}

::ingress_accepted {
    printf("%s", copyinstr(arg0));
}
