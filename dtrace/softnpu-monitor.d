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

::control_dropped {
    printf("control dropped\n");
}

::control_accepted {
    printf("control accepted\n");
}

::control_table_hit {
    printf("%s", copyinstr(arg0));
}

::control_table_miss {
    printf("%s", copyinstr(arg0));
}
