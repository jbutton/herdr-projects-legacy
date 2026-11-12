//! Which GitHub repo gh acts on: origin's, named on every call the board makes (flow/git.rs `gh`). gh's own default is
//! the `upstream` remote where a checkout has one, as a fork's does: there a bare `gh pr view 7` reads upstream's pull
//! request 7 (and one merged there would mark the task done), and `gh pr create` opens nothing.
use super::repos::git_out;
use std::path::Path;
use std::process::{Command, Stdio};

/// A repo on a GitHub host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repo { pub host: String, pub owner: String, pub name: String }
impl Repo {
    /// As gh's `--repo` takes it: `owner/name` on github.com, `host/owner/name` on another host (GitHub Enterprise).
    pub fn arg(&self) -> String { if self.host == "github.com" { format!("{}/{}", self.owner, self.name) } else { format!("{}/{}/{}", self.host, self.owner, self.name) } }
}

/// The repo a remote's URL names, in each form git takes (`https://host/o/r.git`, `ssh://git@host:2222/o/r`,
/// `git@host:o/r.git`, a trailing `/`), and whether it is reached over ssh (its host may then be an alias of
/// `~/.ssh/config`). None for a folder or a `file://` URL.
pub fn parse(url: &str) -> Option<(Repo, bool)> {
    let url = url.trim();
    // With a scheme, or git's short ssh form (`host:path`, no slash before the colon); anything else is a folder.
    let (c, ssh) = if url.contains("://") {
        let c = crate::re!(r"(?i)^([a-z][a-z0-9+.-]*)://(?:[^@/]+@)?([^/:]+)(?::\d*)?/+([^/]+)/([^/]+?)(?:\.git)?/*$").captures(url)?;
        let ssh = c[1].to_lowercase().contains("ssh");
        (c, ssh)
    } else {
        (crate::re!(r"^()(?:[^@/:]+@)?([^/:]+):/*([^/]+)/([^/]+?)(?:\.git)?/*$").captures(url)?, true)
    };
    if c[1].eq_ignore_ascii_case("file") || c[2].starts_with('-') { return None; }
    Some((Repo { host: c[2].to_lowercase(), owner: c[3].to_string(), name: c[4].to_string() }, ssh))
}
/// The host an ssh alias stands for, as ssh itself reads its config (`ssh -G`, which gh asks too): `github-work` may
/// be github.com. The alias itself where ssh cannot say.
fn ssh_host(alias: &str) -> String {
    if alias == "github.com" { return alias.to_string(); }
    let out = Command::new("ssh").args(["-G", "--", alias]).stdin(Stdio::null()).stderr(Stdio::null()).output();
    out.ok().filter(|o| o.status.success()).and_then(|o| String::from_utf8_lossy(&o.stdout).lines().find_map(|l| l.strip_prefix("hostname ").map(|h| h.trim().to_lowercase())))
        .filter(|h| !h.is_empty()).unwrap_or_else(|| alias.to_string())
}
/// Origin's repo: from the URL git uses (`insteadOf` applied), else from the one written in the config (a rewrite to a
/// folder or an ssh alias). None without an origin, or one that names no repo on a host (a folder).
pub fn of_origin(root: &Path) -> Option<Repo> {
    let used = git_out(root, &["remote", "get-url", "origin"]).ok()?;
    let (mut repo, ssh) = parse(&used).or_else(|| git_out(root, &["config", "--get", "remote.origin.url"]).ok().and_then(|w| parse(&w)))?;
    if ssh { repo.host = ssh_host(&repo.host); }
    Some(repo)
}
/// A web link, as a pull request's (`https://github.com/o/r/pull/7`): gh takes its repo from it.
fn is_link(s: &str) -> bool { s.starts_with("https://") || s.starts_with("http://") }
/// `gh api`'s options that take a value: the argument after one is that value, never the endpoint.
const API_VALUED: [&str; 17] = ["-X", "--method", "-H", "--header", "-f", "--raw-field", "-F", "--field", "--input", "-q", "--jq", "-t", "--template", "--hostname", "--cache", "-p", "--preview"];
/// Where gh itself would fill in `{owner}` and `{repo}`: the endpoint and a typed field's value (`-F`), never a `--jq`
/// filter or a raw field (`{owner}` is jq too).
fn placeholders(a: &[String]) -> Vec<usize> {
    let has = |s: &str| s.contains("{owner}") || s.contains("{repo}");
    let (mut out, mut endpoint, mut i) = (vec![], true, 1);
    while i < a.len() {
        let s = a[i].as_str();
        if API_VALUED.contains(&s) {
            if (s == "-F" || s == "--field") && a.get(i + 1).is_some_and(|v| has(v)) { out.push(i + 1); }
            i += 2;
            continue;
        }
        if endpoint && !s.starts_with('-') { endpoint = false; if has(s) { out.push(i); } }
        i += 1;
    }
    out
}
/// What a call needs of a repo: `--repo`, the repo as `repo view`'s argument, the API's placeholders filled in, or none.
enum Need { Nothing, Flag, Positional, Fill(Vec<usize>) }
fn need(a: &[String]) -> Need {
    match (a.first().map(String::as_str).unwrap_or(""), a.get(1).map(String::as_str).unwrap_or("")) {
        ("pr" | "issue" | "run", _) if !a.get(2).is_some_and(|s| is_link(s)) => Need::Flag,
        ("repo", "view") if a.get(2).is_none_or(|s| s.starts_with('-')) => Need::Positional,
        ("api", _) => { let at = placeholders(a); if at.is_empty() { Need::Nothing } else { Need::Fill(at) } }
        _ => Need::Nothing,
    }
}
/// gh's arguments with `repo` named: `--repo` for a pull request, an issue or a run not given by its link, the repo of
/// `repo view`, and the API's `{owner}` and `{repo}` filled in (with `--hostname` off github.com). Anything else
/// (`api user`, a GraphQL node by its id) is about no repo.
pub fn named(args: &[String], repo: &Repo) -> Vec<String> {
    let mut a = args.to_vec();
    match need(&a) {
        // Last, so that it is the one gh takes; a value that reads `--repo` or `-R` (a task's title) is its option's
        // value to gh, never an option of its own.
        Need::Flag => a.extend(["--repo".to_string(), repo.arg()]),
        Need::Positional => a.insert(2, repo.arg()),
        Need::Fill(at) => {
            for i in at {
                a[i] = a[i].replace("{owner}", &repo.owner).replace("{repo}", &repo.name);
                // A typed field would read an owner of digits as a number: filled in, it is a plain string.
                if a[i - 1] == "-F" || a[i - 1] == "--field" { a[i - 1] = "-f".into(); }
            }
            if repo.host != "github.com" { a.extend(["--hostname".to_string(), repo.host.clone()]); }
        }
        Need::Nothing => {}
    }
    a
}
/// gh's arguments for a call run in `root`, with origin's repo named (`named`). Err, and gh is not asked: the call is
/// about a repo, origin names none (a folder, or there is no origin) and the checkout has another remote, which gh
/// would take. With no other remote there is none for gh to take but origin.
pub fn for_gh(args: &[String], root: &Path) -> Result<Vec<String>, String> {
    if matches!(need(args), Need::Nothing) { return Ok(args.to_vec()); }
    if let Some(r) = of_origin(root) { return Ok(named(args, &r)); }
    let others: Vec<String> = git_out(root, &["remote"]).unwrap_or_default().lines().filter(|l| *l != "origin").map(String::from).collect();
    if others.is_empty() { return Ok(args.to_vec()); }
    Err(format!("gh was not asked: origin names no repo on GitHub, and gh would act on another remote's repo instead ({}). Point origin at the repo on GitHub.", others.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn repo(host: &str) -> Repo { Repo { host: host.into(), owner: "o".into(), name: "r".into() } }
    fn args(s: &str) -> Vec<String> { s.split(' ').map(String::from).collect() }
    #[test]
    fn every_form_of_a_remote_names_its_host_owner_and_name() {
        for (url, host, ssh) in [("https://github.com/o/r.git", "github.com", false), ("https://user:token@github.com/o/r", "github.com", false), ("git@github.com:o/r.git", "github.com", true),
            ("ssh://git@github.company.test:2222/o/r.git", "github.company.test", true), ("https://GitHub.company.test/o/r.git/", "github.company.test", false), ("git@github-work:o/r", "github-work", true)] {
            assert_eq!(parse(url), Some((repo(host), ssh)), "{url}");
        }
        for url in ["/tmp/origin.git", "../x/y.git", "file:///tmp/o/r.git", "https://github.com/o", "git@-oProxyCommand=x:o/r"] { assert_eq!(parse(url), None, "{url}"); }
        assert_eq!((repo("github.com").arg(), repo("github.company.test").arg()), ("o/r".to_string(), "github.company.test/o/r".to_string()));
    }
    #[test]
    fn each_call_names_the_repo_the_way_gh_takes_it() {
        let r = repo("github.com");
        for (call, to) in [
            ("pr create --draft --head work/1", "pr create --draft --head work/1 --repo o/r"),
            ("pr list --head work/1 --state all", "pr list --head work/1 --state all --repo o/r"),
            ("issue close 5 --comment Done", "issue close 5 --comment Done --repo o/r"),
            ("run view --job 9 --log-failed", "run view --job 9 --log-failed --repo o/r"),
            ("repo view --json isArchived", "repo view o/r --json isArchived"),
            ("api repos/{owner}/{repo}/actions/permissions --jq .enabled", "api repos/o/r/actions/permissions --jq .enabled"),
            ("api graphql -F owner={owner} -F name={repo} -F number=7", "api graphql -f owner=o -f name=r -F number=7"),
            ("api --paginate repos/{owner}/{repo}/pulls --jq .[]|{owner}", "api --paginate repos/o/r/pulls --jq .[]|{owner}"),
            // A value that reads like the option is that option's value to gh: origin's `--repo` still comes last, and
            // gh takes the last one.
            ("pr create --title --repo --body x", "pr create --title --repo --body x --repo o/r"),
            ("pr create --title -R --body x", "pr create --title -R --body x --repo o/r"),
            ("pr create --title --repo=x/y --body x", "pr create --title --repo=x/y --body x --repo o/r"),
            ("pr view 7 --repo x/y", "pr view 7 --repo x/y --repo o/r"),
            // Named already, or about no repo: as it was.
            ("pr merge https://github.com/x/y/pull/7 --squash", "pr merge https://github.com/x/y/pull/7 --squash"),
            ("repo view x/y", "repo view x/y"),
            ("api user --jq .login", "api user --jq .login"),
            ("api graphql -f id=PR_7", "api graphql -f id=PR_7"),
        ] { assert_eq!(named(&args(call), &r).join(" "), to); }
        let e = repo("github.company.test");
        assert_eq!(named(&args("pr list"), &e).join(" "), "pr list --repo github.company.test/o/r");
        assert_eq!(named(&args("api repos/{owner}/{repo}/status"), &e).join(" "), "api repos/o/r/status --hostname github.company.test");
    }
}
