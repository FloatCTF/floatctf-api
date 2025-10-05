-- for updated_at
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
   NEW."updated_at" = now();
   RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DO $$
DECLARE
    t text;
BEGIN
  -- 遍历 public schema 下所有包含 updated_at 列的表
  FOR t IN
      SELECT table_name
      FROM information_schema.columns
      WHERE table_schema = 'public'
        AND column_name = 'updated_at'
  LOOP
      EXECUTE format(
          'DROP TRIGGER IF EXISTS trg_%I_updated_at ON %I;
           CREATE TRIGGER trg_%I_updated_at
             BEFORE UPDATE ON %I
             FOR EACH ROW
             EXECUTE FUNCTION update_updated_at_column();',
          t, t, t, t
      );
  END LOOP;
END$$;