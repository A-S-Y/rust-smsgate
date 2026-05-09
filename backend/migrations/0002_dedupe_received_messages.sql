WITH ranked AS (
    SELECT
        id,
        row_number() OVER (
            PARTITION BY message_id
            ORDER BY COALESCE(received_at, created_at) DESC, created_at DESC, id DESC
        ) AS row_number
    FROM messages
    WHERE direction = 'received'
      AND message_id IS NOT NULL
)
DELETE FROM messages
WHERE id IN (
    SELECT id
    FROM ranked
    WHERE row_number > 1
);

CREATE UNIQUE INDEX IF NOT EXISTS messages_received_message_id_unique
    ON messages (message_id)
    WHERE direction = 'received' AND message_id IS NOT NULL;
