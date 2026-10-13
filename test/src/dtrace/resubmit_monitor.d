::parser_transition{
    printf("%s\n", copyinstr(arg0));
}

::parser_dropped {
    printf("parser dropped\n");
}

::parser_accepted {
    printf("%s\n", copyinstr(arg0));
}

::control_apply{
    printf("%s\n", copyinstr(arg0));
}

::ingress_dropped {
    printf("ingress dropped\n");
}

::ingress_accepted {
    printf("%s\n", copyinstr(arg0));
}

::control_table_hit {
    printf("%s\n", copyinstr(arg0));
}

::control_table_miss {
    printf("%s\n", copyinstr(arg0));
}

::match_miss {
    printf("%s\n", copyinstr(arg0));
}

::action {
    printf("%s\n", copyinstr(arg0));
}

::resubmit {
    printf("resubmit to program %d\n", arg0);
}

::resubmit_hops_exceeded {
    printf("resubmit hops exceeded for program %d\n", arg0);
}

::resubmit_no_program {
    printf("no program loaded for id %d\n", arg0);
}
