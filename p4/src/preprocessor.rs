use crate::error::PreprocessorError;

#[derive(Clone, Debug)]
struct Macro {
    pub name: String,
    pub body: String,
}

#[derive(Debug)]
pub struct PreprocessorResult {
    pub elements: PreprocessorElements,
    pub lines: Vec<String>,
}

impl Default for PreprocessorResult {
    fn default() -> Self {
        Self{
            elements: PreprocessorElements::default(),
            lines: Vec::new(),
        }
    }
}


#[derive(Debug)]
pub struct PreprocessorElements {
    pub includes: Vec<String>
}

impl Default for PreprocessorElements {
    fn default() -> Self {
        Self{ includes: Vec::new() }
    }
}

pub fn run(source: &str)
-> Result<PreprocessorResult, PreprocessorError> {

    let mut result = PreprocessorResult::default();
    //let mut macros = Vec::new();
    let mut macros_to_process = Vec::new();
    let mut current_macro: Option<Macro> = None;

    //
    // first break the source up into lines
    //
    
    let lines: Vec<&str> = source.lines().collect();
    let mut new_lines: Vec<&str> = Vec::new();

    //
    // process each line of the input
    //
    
    for (i, line) in lines.iter().enumerate() {

        //
        // see if we're in a macro
        //

        match current_macro {
            None => {},
            Some(ref mut m) => {
                if !line.ends_with(r"\") {
                    m.body += &format!("\n{}", line);
                    macros_to_process.push(m.clone());
                    current_macro = None;
                } else {
                    m.body += &format!("\n{}", &line[..line.len()-1]);
                }
                continue;
            }
        }

        //
        // collect includes
        //

        if line.starts_with("#include") {
            process_include(i, line, &mut result)?;
            continue;
        }

        //
        // collect macros
        //

        if line.starts_with("#define") {
            let (name, value) = process_macro_begin(i, line)?;
            let m = Macro{
                name: name,
                body: value,
            };
            if !line.ends_with(r"\") {
                macros_to_process.push(m.clone());
                current_macro = None;
            } else {
                current_macro = Some(m);
            }
            continue;
        }

        //
        // if we are here, this is not a line to be pre-processed
        //
        
        new_lines.push(line)

    }

    //println!("macros to process\n{:#?}", macros_to_process);

    //
    // process macros
    //
    for line in &new_lines {
        let mut l = line.to_string();
        for m in &macros_to_process {
            l = l.replace(&m.name, &m.body);
        }
        result.lines.push(l);
    }

    Ok(result)

}

fn process_include(i: usize, line: &str, result: &mut PreprocessorResult)
-> Result<(), PreprocessorError> {

    let (begin, end) = if let Some(begin) = line.find('<') {
        match line[begin..].find('>') {
            Some(end) => (begin+1, begin+end),
            None => {
                return Err(PreprocessorError{
                    line: i,
                    message: "Unterminated '<'".into(),
                    source: line.to_string(),
                })
            }
        }
    } else if let Some(begin) = line.find('"') {
        match line[begin..].find('"') {
            Some(end) => (begin+1, begin+end),
            None => {
                return Err(PreprocessorError{
                    line: i,
                    message: "Unterminated '\"'".into(),
                    source: line.to_string(),
                })
            }
        }
    } else {
        return Err(PreprocessorError{
            line: i,
            message: "Invalid #include".into(),
            source: line.to_string(),
        })
    };

    if end < line.len() {
        for c in line[end+1..].chars() {
            if !c.is_whitespace() {
                return Err(PreprocessorError{
                    line: i,
                    message: format!(
                        "Unexpected character after #include '{}'",
                        c,
                    ),
                    source: line.to_string(),
                })
            }
        }
    }
    result.elements.includes.push(line[begin..end].into());

    Ok(())

}

fn process_macro_begin(i: usize, line: &str)
-> Result<(String, String), PreprocessorError> {

    let mut parts = line.split_whitespace();
    // discard #define
    parts.next();

    let name = match parts.next() {
        Some(n) => n.into(),
        None => {
            return Err(PreprocessorError{
                line: i,
                message: "Macros must have a name".into(),
                source: line.to_string(),
            })
        }
    };

    let value = match parts.next() {
        Some(v) => v.into(),
        None => "".into(),
    };

    Ok((name, value))

}
