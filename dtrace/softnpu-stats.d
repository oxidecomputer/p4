::parser_transition{
    @stats["parser transition", copyinstr(arg0)] = count();
}

::parser_dropped {
    @stats["parser", "drop"] = count();
}

::parser_accepted {
    @stats["parser", "accept"] = count();
}

::control_apply{
    @stats["control apply", copyinstr(arg0)]= count();
}

::control_dropped {
    @stats["control", "drop"] = count();
}

::control_accepted {
    @stats["control", "accept"] = count();
}

::control_table_hit {
    @stats["table hit", copyinstr(arg0)] = count();
}

::control_table_miss {
    @stats["table miss", copyinstr(arg0)] = count();
}

tick-10sec
{
    printa(@stats);
    printf("==========================================================================================================================\n");
}
