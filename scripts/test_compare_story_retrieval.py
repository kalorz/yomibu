import contextlib
import io
import json
import unittest
from unittest import mock

import compare_story_retrieval as compare


class RetrievalComparisonTests(unittest.TestCase):
    def test_only_the_embedding_credential_reaches_child_commands(self):
        environment = {
            "PATH": "/synthetic/bin",
            "YOMIBU_HTTP_EMBEDDINGS_API_KEY": "synthetic-embedding-key",
            "YOMIBU_OPENAI_API_KEY": "unrelated-generation-key",
            "YOMIBU_CONFIG": "unrelated.toml",
        }
        with (
            mock.patch.object(compare, "os") as process,
            mock.patch.object(compare, "sys") as command,
            mock.patch.object(compare, "subprocess") as child,
            contextlib.redirect_stdout(io.StringIO()) as output,
        ):
            process.environ = environment
            command.argv = ["compare", "yomibu", "--embedding-provider", "openai",
                            "--allow-embedding-call"]
            child.check_output.return_value = json.dumps({
                "selection": {"vocabulary_ids": [], "embedding_model": "synthetic"}
            }).encode()
            compare.main()
        rows = json.loads(output.getvalue())["cases"]
        self.assertGreater(len(rows), 0)
        for operation in (child.run, child.check_output):
            self.assertEqual(operation.call_count, len(rows))
            for call in operation.call_args_list:
                self.assertEqual(call.kwargs["env"], {
                    "PATH": "/synthetic/bin",
                    "YOMIBU_HTTP_EMBEDDINGS_API_KEY": "synthetic-embedding-key",
                })


if __name__ == "__main__":
    unittest.main()
