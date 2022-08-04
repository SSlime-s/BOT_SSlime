DROP DATABASE IF EXISTS markov;
CREATE DATABASE markov;
USE markov;

CREATE TABLE IF NOT EXISTS `markov_cache` (
  `cache`       LONGTEXT NOT NULL,
  `last_update` DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `messages` (
  `id`         CHAR(36) NOT NULL,
  `channel_id` CHAR(36) NOT NULL,
  `content`    TEXT NOT NULL,
  `created_at` DATETIME NOT NULL,
  PRIMARY KEY (id)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `frequency` (
  `channel_id` CHAR(36) NOT NULL,
  `frequency`  INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (channel_id)
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_unicode_ci;
