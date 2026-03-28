use conch_parser::ast::{
    AndOr as ParsedAndOr, Command as ParsedCommand, ComplexWord as ParsedComplexWord,
    DefaultAndOrList as ParsedAndOrList, DefaultListableCommand as ParsedListableCommand,
    DefaultParameterSubstitution as ParsedParameterSubstitution,
    DefaultPipeableCommand as ParsedPipeableCommand, DefaultSimpleCommand as ParsedSimpleCommand,
    DefaultSimpleWord as ParsedSimpleWord, DefaultWord as ParsedWord,
    ListableCommand as ParsedListableCommandKind,
    ParameterSubstitution as ParsedParameterSubstitutionKind,
    PipeableCommand as ParsedPipeableCommandKind, RedirectOrCmdWord as ParsedRedirectOrCmdWord,
    RedirectOrEnvVar as ParsedRedirectOrEnvVar, SimpleWord as ParsedSimpleWordKind,
    TopLevelCommand as ParsedTopLevelCommand, TopLevelWord as ParsedTopLevelWord,
    Word as ParsedWordKind,
};
use conch_parser::lexer::Lexer as ShellLexer;
use conch_parser::parse::DefaultParser as ShellParser;

use super::shell::{has_unsupported_shell_quoting, needs_shell_parser_fallback};

pub(super) fn parser_allows_rewrite_shape(cmd: &str) -> bool {
    let trimmed = cmd.trim();
    if trimmed.is_empty()
        || trimmed.contains('\n')
        || trimmed.contains('\r')
        || has_unsupported_shell_quoting(trimmed)
    {
        return false;
    }

    let mut parser = ShellParser::new(ShellLexer::new(trimmed.chars()));

    loop {
        match parser.complete_command() {
            Ok(Some(command)) => {
                if !parsed_top_level_command_is_safe(&command) {
                    return false;
                }
            }
            Ok(None) => return true,
            Err(_) => return false,
        }
    }
}

pub(super) fn rewrite_shape_requires_parser(cmd: &str) -> bool {
    needs_shell_parser_fallback(cmd)
}

fn parsed_top_level_command_is_safe(command: &ParsedTopLevelCommand<String>) -> bool {
    match &command.0 {
        ParsedCommand::Job(_) => false,
        ParsedCommand::List(list) => parsed_and_or_list_is_safe(list),
    }
}

fn parsed_and_or_list_is_safe(list: &ParsedAndOrList) -> bool {
    parsed_listable_command_is_safe(&list.first)
        && list.rest.iter().all(|item| match item {
            ParsedAndOr::And(command) | ParsedAndOr::Or(command) => {
                parsed_listable_command_is_safe(command)
            }
        })
}

fn parsed_listable_command_is_safe(command: &ParsedListableCommand) -> bool {
    match command {
        ParsedListableCommandKind::Single(pipeable) => parsed_pipeable_command_is_safe(pipeable),
        ParsedListableCommandKind::Pipe(_, _) => false,
    }
}

fn parsed_pipeable_command_is_safe(command: &ParsedPipeableCommand) -> bool {
    match command {
        ParsedPipeableCommandKind::Simple(simple) => parsed_simple_command_is_safe(simple),
        ParsedPipeableCommandKind::Compound(_) | ParsedPipeableCommandKind::FunctionDef(_, _) => {
            false
        }
    }
}

fn parsed_simple_command_is_safe(command: &ParsedSimpleCommand) -> bool {
    command
        .redirects_or_env_vars
        .iter()
        .all(|entry| match entry {
            ParsedRedirectOrEnvVar::EnvVar(_, word) => {
                word.as_ref().is_none_or(parsed_top_level_word_is_safe)
            }
            ParsedRedirectOrEnvVar::Redirect(_) => false,
        })
        && command
            .redirects_or_cmd_words
            .iter()
            .all(|entry| match entry {
                ParsedRedirectOrCmdWord::CmdWord(word) => parsed_top_level_word_is_safe(word),
                ParsedRedirectOrCmdWord::Redirect(_) => false,
            })
}

fn parsed_top_level_word_is_safe(word: &ParsedTopLevelWord<String>) -> bool {
    match &word.0 {
        ParsedComplexWord::Concat(parts) => parts.iter().all(parsed_word_is_safe),
        ParsedComplexWord::Single(part) => parsed_word_is_safe(part),
    }
}

fn parsed_word_is_safe(word: &ParsedWord) -> bool {
    match word {
        ParsedWordKind::Simple(simple) => parsed_simple_word_is_safe(simple),
        ParsedWordKind::DoubleQuoted(words) => words.iter().all(parsed_simple_word_is_safe),
        ParsedWordKind::SingleQuoted(_) => true,
    }
}

fn parsed_simple_word_is_safe(word: &ParsedSimpleWord) -> bool {
    match word {
        ParsedSimpleWordKind::Literal(_)
        | ParsedSimpleWordKind::Escaped(_)
        | ParsedSimpleWordKind::Param(_)
        | ParsedSimpleWordKind::Star
        | ParsedSimpleWordKind::Question
        | ParsedSimpleWordKind::SquareOpen
        | ParsedSimpleWordKind::SquareClose
        | ParsedSimpleWordKind::Tilde
        | ParsedSimpleWordKind::Colon => true,
        ParsedSimpleWordKind::Subst(subst) => parsed_parameter_substitution_is_safe(subst),
    }
}

fn parsed_parameter_substitution_is_safe(subst: &ParsedParameterSubstitution) -> bool {
    match subst {
        ParsedParameterSubstitutionKind::Command(_) | ParsedParameterSubstitutionKind::Arith(_) => {
            false
        }
        ParsedParameterSubstitutionKind::Len(_) => true,
        ParsedParameterSubstitutionKind::Default(_, _, word)
        | ParsedParameterSubstitutionKind::Assign(_, _, word)
        | ParsedParameterSubstitutionKind::Error(_, _, word)
        | ParsedParameterSubstitutionKind::Alternative(_, _, word)
        | ParsedParameterSubstitutionKind::RemoveSmallestSuffix(_, word)
        | ParsedParameterSubstitutionKind::RemoveLargestSuffix(_, word)
        | ParsedParameterSubstitutionKind::RemoveSmallestPrefix(_, word)
        | ParsedParameterSubstitutionKind::RemoveLargestPrefix(_, word) => {
            word.as_ref().is_none_or(parsed_top_level_word_is_safe)
        }
    }
}
