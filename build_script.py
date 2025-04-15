import subprocess
import os
import sys
import uuid # used for generating a unique container name
import shlex # used for printing commands safely

# --- configuration ---
IMAGE_NAME = "git-changes-rs-app"
LINUX_BIN_NAME = "git-changes-rs-linux"
WINDOWS_BIN_NAME = "git-changes-rs.exe"
# generate a unique name for the temporary container to avoid conflicts
TEMP_CONTAINER_NAME = f"temp-extractor-{uuid.uuid4()}"

def run_command(command_list, check=True, suppress_output=False):
    """
    helper function to run an external command.

    Args:
        command_list: a list of strings representing the command and args.
        check: if true, raise subprocess.calledprocesserror on non-zero exit.
        suppress_output: if true, hide stdout/stderr unless an error occurs.

    Returns:
        true if the command succeeded, false otherwise.
    """
    command_str = shlex.join(command_list) # safely join for printing
    print(f">>> running: {command_str}")
    try:
        stdout_pipe = subprocess.PIPE if suppress_output else None
        stderr_pipe = subprocess.PIPE if suppress_output else None

        # run the command
        result = subprocess.run(
            command_list,
            check=check,        # raise exception if command fails
            capture_output=suppress_output, # capture output only if suppressing
            text=True,          # decode output as text
            encoding='utf-8'    # specify encoding for robustness
        )
        # if suppressing output and there was stderr, print it
        if suppress_output and result.stderr:
             print(f"stderr: {result.stderr.strip()}", file=sys.stderr)
        return True # indicate success

    except subprocess.CalledProcessError as e:
        # error is raised if check=true and command returns non-zero
        print(f"error: command failed with exit code {e.returncode}", file=sys.stderr)
        print(f"command: {command_str}", file=sys.stderr)
        # print captured output if available
        if e.stdout:
            print(f"stdout:\n{e.stdout.strip()}", file=sys.stderr)
        if e.stderr:
            print(f"stderr:\n{e.stderr.strip()}", file=sys.stderr)
        return False # indicate failure
    except FileNotFoundError:
        # error if the command (e.g., 'docker') isn't found
        print(f"error: command not found: '{command_list[0]}'. is docker installed and in your system's path?", file=sys.stderr)
        return False
    except Exception as e:
        # catch any other unexpected errors during subprocess execution
        print(f"an unexpected error occurred running command: {command_str}", file=sys.stderr)
        print(e, file=sys.stderr)
        return False

def main():
    """main function to orchestrate the build and extraction."""
    print("\n=== starting docker build script (python) ===")

    # check if dockerfile exists in the current directory
    if not os.path.exists("Dockerfile"):
        print("error: dockerfile not found in the current directory.", file=sys.stderr)
        print("please run this script from the project root.", file=sys.stderr)
        sys.exit(1) # exit with error code 1

    # --- build the image ---
    build_command = ["docker", "build", "-t", IMAGE_NAME, "."]
    if not run_command(build_command):
         sys.exit(1) # exit if build fails

    # --- create container and extract files ---
    container_created = False
    exit_code = 0 # assume success initially
    try:
        # create container command
        create_command = ["docker", "create", "--name", TEMP_CONTAINER_NAME, IMAGE_NAME]
        # run create command, suppress the container id output
        if not run_command(create_command, suppress_output=True):
             # error already printed by run_command
             raise RuntimeError("failed to create temporary container.")
        container_created = True
        print(f">>> created temporary container: {TEMP_CONTAINER_NAME}")

        # extract linux binary command
        cp_linux_command = [
            "docker", "cp",
            f"{TEMP_CONTAINER_NAME}:/usr/local/bin/{LINUX_BIN_NAME}",
            f"./{LINUX_BIN_NAME}"
        ]
        if not run_command(cp_linux_command):
            raise RuntimeError("failed to copy linux binary.")

        # extract windows binary command
        cp_windows_command = [
            "docker", "cp",
            f"{TEMP_CONTAINER_NAME}:/usr/local/bin/{WINDOWS_BIN_NAME}",
            f"./{WINDOWS_BIN_NAME}"
        ]
        if not run_command(cp_windows_command):
             raise RuntimeError("failed to copy windows binary.")

        print("\n>>> build and extraction complete!")
        print(f"binaries created: ./{LINUX_BIN_NAME}, ./{WINDOWS_BIN_NAME}")

    except Exception as e:
         print(f"\nan error occurred during extraction: {e}", file=sys.stderr)
         exit_code = 1 # set error exit code

    finally:
        # --- cleanup ---
        # always attempt to remove the container if it was created
        if container_created:
            print(f">>> cleaning up container {TEMP_CONTAINER_NAME}...")
            rm_command = ["docker", "rm", TEMP_CONTAINER_NAME]
            # run cleanup command but don't exit script if cleanup fails, just report
            if not run_command(rm_command, check=False, suppress_output=True):
                 print(f"warning: failed to remove temporary container '{TEMP_CONTAINER_NAME}'. you may need to remove it manually.", file=sys.stderr)
            else:
                 print("cleanup finished.")
        print("\n=== script finished ===")
        sys.exit(exit_code) # exit with 0 on success, 1 on error

# standard python entry point guard
if __name__ == "__main__":
    main()
