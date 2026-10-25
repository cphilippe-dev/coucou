// Accepter automatiquement les demandes d'autorisation pendant une durée choisie.
//
// Choisi depuis le menu de l'icône Coucou (zone de notification) :
// « Accepter auto. » → 1 h / 2 h / 4 h / 8 h / Arrêter.
//
// Pendant la fenêtre active, chaque PermissionRequest reçu par pipe.rs reçoit
// « allow » tout de suite, sans carte sur l'île. Deux exceptions restent toujours
// manuelles : les questions de Claude (AskUserQuestion) et la validation d'un plan
// (ExitPlanMode) — ce ne sont pas des autorisations, ce sont des choix.
//
// Rien n'est enregistré sur le disque : quitter Coucou coupe le mode.
// Les règles « deny » de ~/.claude/settings.json s'appliquent toujours avant le hook,
// donc ce qui y est interdit reste interdit.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

/// Fin de la fenêtre, en secondes Unix. 0 = désactivé.
static UNTIL: AtomicU64 = AtomicU64::new(0);

/// Outils jamais acceptés automatiquement.
const ALWAYS_MANUAL: [&str; 2] = ["AskUserQuestion", "ExitPlanMode"];

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Active le mode pour `hours` heures à partir de maintenant (remplace une fenêtre en cours).
/// Renvoie l'heure de fin (secondes Unix).
pub fn start(hours: u64) -> u64 {
    let until = now() + hours * 3600;
    UNTIL.store(until, Ordering::SeqCst);
    until
}

pub fn stop() {
    UNTIL.store(0, Ordering::SeqCst);
}

/// Secondes restantes, 0 si inactif ou expiré.
pub fn remaining() -> u64 {
    UNTIL.load(Ordering::SeqCst).saturating_sub(now())
}

/// Heure de fin en cours (0 si aucune) — sert à savoir si une fenêtre a été remplacée.
pub fn until() -> u64 {
    UNTIL.load(Ordering::SeqCst)
}

/// Ce PermissionRequest doit-il être accepté sans demander ?
pub fn should_allow(payload: &Value) -> bool {
    if remaining() == 0 {
        return false;
    }
    let tool = payload.get("tool_name").and_then(Value::as_str).unwrap_or_default();
    !ALWAYS_MANUAL.contains(&tool)
}

/// Texte d'état pour le menu : « Accepter auto. : inactif » ou « … : encore 3 h 12 ».
pub fn status_label() -> String {
    let left = remaining();
    if left == 0 {
        return "Accepter auto. : inactif".to_string();
    }
    let minutes = left.div_ceil(60);
    let (h, m) = (minutes / 60, minutes % 60);
    if h == 0 {
        format!("Accepter auto. : encore {m} min")
    } else {
        format!("Accepter auto. : encore {h} h {m:02}")
    }
}

/// Délai jusqu'à la fin de la fenêtre en cours.
pub fn time_left() -> Duration {
    Duration::from_secs(remaining())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn window_controls_decisions() {
        stop();
        let bash = json!({ "tool_name": "Bash" });
        assert!(!should_allow(&bash), "inactif : on demande");

        start(1);
        assert!(should_allow(&bash), "actif : accepté");
        assert!(!should_allow(&json!({ "tool_name": "AskUserQuestion" })), "questions toujours manuelles");
        assert!(!should_allow(&json!({ "tool_name": "ExitPlanMode" })), "plans toujours manuels");
        assert!(remaining() > 3590 && remaining() <= 3600);
        assert!(status_label().starts_with("Accepter auto. : encore 1 h"));

        stop();
        assert!(!should_allow(&bash));
        assert_eq!(status_label(), "Accepter auto. : inactif");
    }
}
