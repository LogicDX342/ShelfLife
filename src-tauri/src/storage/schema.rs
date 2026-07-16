diesel::table! {
    app_config (id) {
        id -> Integer,
        default_ttl_seconds -> BigInt,
        stale_threshold_seconds -> BigInt,
        decaying_threshold_seconds -> BigInt,
        default_move_destination -> Nullable<Text>,
        notifications_enabled -> Bool,
        start_at_login -> Bool,
        close_behavior -> Text,
        dropzone_enabled -> Bool,
    }
}

diesel::table! {
    watch_targets (id) {
        id -> Text,
        ordinal -> BigInt,
        path -> Text,
        enabled -> Bool,
        recursive -> Bool,
    }
}

diesel::table! {
    watch_target_ignore_patterns (target_id, ordinal) {
        target_id -> Text,
        ordinal -> BigInt,
        value -> Text,
    }
}

diesel::table! {
    automation_rules (id) {
        id -> Text,
        name -> Text,
        enabled -> Bool,
        priority -> Integer,
        watch_path -> Text,
        ttl_seconds -> BigInt,
        mode -> Text,
        created_at -> BigInt,
        updated_at -> BigInt,
        action_kind -> Text,
        action_destination_folder -> Nullable<Text>,
        action_rename_template -> Nullable<Text>,
        size_kind -> Text,
        size_min -> Nullable<BigInt>,
        size_max -> Nullable<BigInt>,
    }
}

diesel::table! {
    rule_extensions (rule_id, ordinal) {
        rule_id -> Text,
        ordinal -> BigInt,
        value -> Text,
    }
}

diesel::table! {
    rule_filename_globs (rule_id, ordinal) {
        rule_id -> Text,
        ordinal -> BigInt,
        value -> Text,
    }
}

diesel::table! {
    rule_filename_regexes (rule_id, ordinal) {
        rule_id -> Text,
        ordinal -> BigInt,
        value -> Text,
    }
}

diesel::table! {
    rule_source_domains (rule_id, ordinal) {
        rule_id -> Text,
        ordinal -> BigInt,
        value -> Text,
    }
}

diesel::table! {
    tracked_files (path) {
        path -> Text,
        file_name -> Text,
        watch_target_id -> Text,
        size_bytes -> BigInt,
        first_seen_at -> BigInt,
        last_observed_mtime -> Nullable<BigInt>,
        last_observed_atime -> Nullable<BigInt>,
        last_user_action_at -> Nullable<BigInt>,
        freshness_at -> BigInt,
        expiry_kind -> Text,
        expires_at -> Nullable<BigInt>,
        state -> Text,
        origin_url -> Nullable<Text>,
    }
}

diesel::table! {
    tracked_file_rules (file_path, ordinal) {
        file_path -> Text,
        ordinal -> BigInt,
        rule_id -> Text,
    }
}

diesel::table! {
    audit_sequence_state (id) {
        id -> Integer,
        next_sequence -> BigInt,
    }
}

diesel::table! {
    audit_entries (sequence) {
        sequence -> BigInt,
        id -> Text,
        timestamp -> BigInt,
        action_kind -> Text,
        source_path -> Text,
        destination_path -> Nullable<Text>,
        file_name -> Text,
        size_bytes -> BigInt,
        rule_id -> Nullable<Text>,
        rule_name -> Nullable<Text>,
        undo_status_kind -> Text,
        undo_status_reason -> Nullable<Text>,
        explanation_file_path -> Nullable<Text>,
        explanation_size_bytes -> Nullable<BigInt>,
        explanation_rule_id -> Nullable<Text>,
        explanation_rule_name -> Nullable<Text>,
        explanation_matched_extension -> Nullable<Bool>,
        explanation_matched_size -> Nullable<Bool>,
        explanation_matched_origin -> Nullable<Text>,
        explanation_matched_filename_pattern -> Nullable<Text>,
        explanation_proposed_action_kind -> Nullable<Text>,
        explanation_proposed_action_destination_folder -> Nullable<Text>,
        explanation_proposed_action_rename_template -> Nullable<Text>,
        explanation_mode -> Nullable<Text>,
        explanation_message -> Nullable<Text>,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    app_config,
    watch_targets,
    watch_target_ignore_patterns,
    automation_rules,
    rule_extensions,
    rule_filename_globs,
    rule_filename_regexes,
    rule_source_domains,
    tracked_files,
    tracked_file_rules,
    audit_sequence_state,
    audit_entries,
);
